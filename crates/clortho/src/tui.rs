//! Terminal client: the pending queue, who is asking, and a masked prompt.

use crate::cli::{age, outcome_str};
use crate::client;
use clortho_core::Secret;
use clortho_core::proto::*;
use clortho_core::transport::{self, UnixStream};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::io;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use zeroize::Zeroize;

enum Mode {
    List,
    Input {
        id: u64,
        buf: String,
        first: Option<String>,
        remember: bool,
    },
}

struct App {
    writer: UnixStream,
    daemon_pid: u32,
    policy: String,
    pending: Vec<RequestView>,
    recent: Vec<HistoryEntry>,
    selected: Option<u64>,
    mode: Mode,
    status: String,
    once: bool,
    quit: bool,
}

impl App {
    fn sel_index(&self) -> Option<usize> {
        let id = self.selected?;
        self.pending.iter().position(|r| r.id == id)
    }

    fn current(&self) -> Option<&RequestView> {
        self.sel_index().map(|i| &self.pending[i])
    }

    fn can_remember(&self, r: &RequestView) -> bool {
        self.policy != "off" && r.prompt.allow_store && !r.prompt.repeat && !r.prompt.echo
    }

    fn leave_input(&mut self) {
        if let Mode::Input { buf, first, .. } = &mut self.mode {
            buf.zeroize();
            if let Some(f) = first {
                f.zeroize();
            }
        }
        self.mode = Mode::List;
    }

    fn apply_snapshot(&mut self, pending: Vec<RequestView>, recent: Vec<HistoryEntry>) {
        let old_index = self.sel_index().unwrap_or(0);
        self.pending = pending;
        self.recent = recent;
        if self.sel_index().is_none() {
            let i = old_index.min(self.pending.len().saturating_sub(1));
            self.selected = self.pending.get(i).map(|r| r.id);
        }
        if let Mode::Input { id, .. } = self.mode
            && !self.pending.iter().any(|r| r.id == id)
        {
            self.leave_input();
            self.status = format!("request #{id} was resolved elsewhere");
        }
        if self.once && self.pending.is_empty() {
            self.quit = true;
        }
    }

    fn send(&mut self, id: u64, reply: Reply) {
        if let Err(e) = transport::send(&mut self.writer, &ClientMsg::Answer { id, reply }) {
            self.status = format!("send failed: {e}");
        }
    }

    fn move_sel(&mut self, delta: isize) {
        if self.pending.is_empty() {
            return;
        }
        let i = self.sel_index().unwrap_or(0) as isize + delta;
        let i = i.clamp(0, self.pending.len() as isize - 1) as usize;
        self.selected = Some(self.pending[i].id);
    }

    fn key_list(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            _ => {}
        }
        let Some(r) = self.current().cloned() else {
            return;
        };
        self.status.clear();
        match key.code {
            KeyCode::Enter => match r.prompt.kind {
                Kind::Pin => {
                    self.mode = Mode::Input {
                        id: r.id,
                        buf: String::new(),
                        first: None,
                        remember: false,
                    }
                }
                _ => self.send(r.id, Reply::Confirm),
            },
            KeyCode::Char('u') if r.prompt.kind == Kind::Pin => {
                if r.stored {
                    self.send(r.id, Reply::UseStored)
                } else {
                    self.status = "nothing stored for this key".into()
                }
            }
            KeyCode::Char('y') if r.prompt.kind != Kind::Pin => self.send(r.id, Reply::Confirm),
            KeyCode::Char('n') if r.prompt.kind == Kind::Confirm => self.send(r.id, Reply::Deny),
            KeyCode::Char('x') | KeyCode::Delete => self.send(r.id, Reply::Cancel),
            KeyCode::Char('f') if r.stored => self.send(r.id, Reply::Forget),
            _ => {}
        }
    }

    fn key_input(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Mode::Input { id, .. } = self.mode else {
            return;
        };
        let Some(req) = self.pending.iter().find(|r| r.id == id).cloned() else {
            self.leave_input();
            return;
        };
        let can_remember = self.can_remember(&req);
        let Mode::Input {
            buf,
            first,
            remember,
            ..
        } = &mut self.mode
        else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.leave_input(),
            KeyCode::Char('c') if ctrl => self.leave_input(),
            KeyCode::Char('u') if ctrl => buf.zeroize(),
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Tab if can_remember => *remember = !*remember,
            KeyCode::Char(c) if !ctrl => buf.push(c),
            KeyCode::Enter => {
                if req.prompt.repeat && first.is_none() {
                    *first = Some(std::mem::take(buf));
                    return;
                }
                if let Some(f) = first
                    && f != buf
                {
                    f.zeroize();
                    buf.zeroize();
                    *first = None;
                    self.status = "entries did not match; start again".into();
                    return;
                }
                let secret = Secret::new(buf.as_str());
                let remember = *remember && can_remember;
                self.leave_input();
                self.send(id, Reply::Secret { secret, remember });
            }
            _ => {}
        }
    }
}

fn source_color(s: Source) -> Color {
    match s {
        Source::Gpg => Color::Cyan,
        Source::Ssh => Color::Green,
        Source::Sudo => Color::Red,
        Source::Git => Color::Yellow,
        _ => Color::Magenta,
    }
}

fn draw(f: &mut Frame, app: &App) {
    let n = app.pending.len() as u16;
    let [head, list, detail, recent, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length((n + 2).clamp(3, 9)),
        Constraint::Min(6),
        Constraint::Length(8),
        Constraint::Length(1),
    ])
    .areas(f.area());

    let dim = Style::default().fg(Color::DarkGray);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    f.render_widget(
        Line::from(vec![
            Span::styled(" clortho ", bold.fg(Color::Black).bg(Color::Yellow)),
            Span::raw(format!("  {} pending", app.pending.len())),
            Span::styled(
                format!("   store: {}   daemon pid {}", app.policy, app.daemon_pid),
                dim,
            ),
        ]),
        head,
    );

    let items: Vec<ListItem> = app
        .pending
        .iter()
        .map(|r| {
            let mut spans = vec![
                Span::styled(format!("{:>4} ", age(r.created_ms)), dim),
                Span::styled(
                    format!("{:<8} ", r.prompt.source.as_str()),
                    Style::default().fg(source_color(r.prompt.source)),
                ),
                Span::raw(r.prompt.summary()),
                Span::styled(format!("  {}", r.origin.who()), dim),
            ];
            if r.stored {
                spans.push(Span::styled(
                    "  ● stored",
                    Style::default().fg(Color::Green),
                ));
            }
            if r.prompt.error.is_some() {
                spans.push(Span::styled("  retry", Style::default().fg(Color::Red)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let empty = items.is_empty();
    let mut state = ListState::default().with_selected(app.sel_index());
    let listw = List::new(items)
        .block(Block::bordered().title(" Pending "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(listw, list, &mut state);
    if empty {
        let inner = Rect {
            x: list.x + 2,
            y: list.y + 1,
            width: list.width.saturating_sub(4),
            height: 1,
        };
        f.render_widget(Span::styled("Nothing is waiting for you.", dim), inner);
    }

    f.render_widget(
        Paragraph::new(detail_lines(app))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" Request ")),
        detail,
    );

    let hist: Vec<ListItem> = app
        .recent
        .iter()
        .map(|h| {
            let color = match h.outcome {
                Outcome::Answered | Outcome::Confirmed | Outcome::FromStore => Color::Green,
                Outcome::Denied | Outcome::Cancelled => Color::Red,
                _ => Color::DarkGray,
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>4} ", age(h.resolved_ms)), dim),
                Span::styled(
                    format!("{:<11}", outcome_str(h.outcome)),
                    Style::default().fg(color),
                ),
                Span::styled(format!("{:<8} ", h.request.prompt.source.as_str()), dim),
                Span::raw(h.request.prompt.summary()),
                Span::styled(format!("  by {}", h.by), dim),
            ]))
        })
        .collect();
    f.render_widget(
        List::new(hist).block(Block::bordered().title(" Recent ")),
        recent,
    );

    let help = if !app.status.is_empty() {
        Span::styled(
            format!(" {}", app.status),
            Style::default().fg(Color::Yellow),
        )
    } else {
        let keys = match (&app.mode, app.current()) {
            (Mode::Input { .. }, _) => " enter submit · tab remember · ctrl-u clear · esc back",
            (_, Some(r)) if r.prompt.kind == Kind::Pin => {
                " enter type secret · u use stored · f forget stored · x cancel · j/k move · q quit"
            }
            (_, Some(r)) if r.prompt.kind == Kind::Confirm => {
                " y/enter yes · n no · x cancel · j/k move · q quit"
            }
            (_, Some(_)) => " enter acknowledge · x cancel · j/k move · q quit",
            _ => " q quit",
        };
        Span::styled(keys, dim)
    };
    f.render_widget(help, foot);

    if let Mode::Input {
        id,
        buf,
        first,
        remember,
    } = &app.mode
        && let Some(r) = app.pending.iter().find(|r| r.id == *id)
    {
        draw_input(f, app, r, buf, first.is_some(), *remember);
    }
}

fn detail_lines(app: &App) -> Vec<Line<'static>> {
    let Some(r) = app.current() else {
        return vec![];
    };
    let label = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!("{k:<9}"), Style::default().fg(Color::DarkGray)),
            Span::raw(v),
        ])
    };
    let mut out = Vec::new();
    if let Some(e) = &r.prompt.error {
        out.push(Line::styled(
            e.clone(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(d) = &r.prompt.description {
        out.extend(d.lines().map(|l| Line::raw(l.to_string())));
        out.push(Line::raw(""));
    }
    if let Some(k) = &r.prompt.key {
        out.push(label("key", k.clone()));
    }
    let same = app
        .pending
        .iter()
        .filter(|o| o.group == r.group && o.id != r.id)
        .count();
    if same > 0 {
        out.push(label(
            "also",
            format!("{same} other waiting request(s) for this key get the same answer"),
        ));
    }
    for (i, p) in r.origin.chain.iter().enumerate() {
        let k = if i == 0 { "asked by" } else { "  via" };
        out.push(label(k, format!("{} [{}]  {}", p.name, p.pid, p.cmdline)));
    }
    if r.origin.chain.is_empty() {
        out.push(label("asked by", r.origin.who()));
    }
    if let Some(t) = &r.origin.tty {
        out.push(label("tty", t.clone()));
    }
    if let Some(c) = &r.origin.cwd {
        out.push(label("cwd", c.clone()));
    }
    if let Some(h) = &r.origin.host {
        out.push(label("host", h.clone()));
    }
    if let Some(t) = r.prompt.timeout_secs {
        out.push(label("timeout", format!("{t}s")));
    }
    out
}

fn draw_input(f: &mut Frame, app: &App, r: &RequestView, buf: &str, second: bool, remember: bool) {
    let area = f.area();
    let w = area.width.saturating_sub(4).min(64);
    let h = 7u16.min(area.height);
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let shown: String = if r.prompt.echo {
        buf.to_string()
    } else {
        "•".repeat(buf.chars().count())
    };
    let label = if second {
        "Again".to_string()
    } else {
        r.prompt.prompt.clone().unwrap_or_else(|| {
            if r.prompt.echo {
                "Answer:".into()
            } else {
                "Secret:".into()
            }
        })
    };
    let mut lines = vec![
        Line::raw(r.prompt.summary()),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                format!("{} ", label.trim_end()),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(shown),
            Span::styled("▏", Style::default().fg(Color::Yellow)),
        ]),
        Line::raw(""),
    ];
    if app.can_remember(r) {
        lines.push(Line::raw(format!(
            "[{}] remember in the system key store (tab)",
            if remember { "x" } else { " " }
        )));
    }
    let title = format!(" {} #{} ", r.prompt.source.as_str(), r.id);
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        rect,
    );
}

fn event_loop(
    term: &mut DefaultTerminal,
    app: &mut App,
    rx: Receiver<Option<ServerMsg>>,
) -> io::Result<()> {
    while !app.quit {
        term.draw(|f| draw(f, app))?;
        if event::poll(Duration::from_millis(150))?
            && let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
        {
            match app.mode {
                Mode::List => app.key_list(key),
                Mode::Input { .. } => app.key_input(key),
            }
        }
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Some(ServerMsg::Snapshot { pending, recent }) => {
                    app.apply_snapshot(pending, recent)
                }
                Some(ServerMsg::Error { message }) => app.status = message,
                Some(_) => {}
                None => {
                    app.status = "daemon went away".into();
                    app.quit = true;
                }
            }
        }
    }
    app.leave_input();
    Ok(())
}

pub fn run(once: bool) -> i32 {
    let conn = match client::connect_or_spawn(Role::Ui, "tui", false) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("clortho: cannot reach the daemon: {e}");
            return 1;
        }
    };
    let client::Conn {
        mut reader,
        writer,
        daemon_pid,
        store_policy,
    } = conn;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            match transport::recv::<ServerMsg>(&mut reader) {
                Ok(Some(m)) => {
                    if tx.send(Some(m)).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });
    let mut app = App {
        writer,
        daemon_pid,
        policy: store_policy,
        pending: vec![],
        recent: vec![],
        selected: None,
        mode: Mode::List,
        status: String::new(),
        once,
        quit: false,
    };
    // Apply the initial snapshot before drawing so --once can exit at once.
    if let Ok(Some(ServerMsg::Snapshot { pending, recent })) =
        rx.recv_timeout(Duration::from_secs(2))
    {
        app.apply_snapshot(pending, recent);
    }
    if app.quit {
        println!("clortho: nothing pending");
        return 0;
    }
    let mut term = ratatui::init();
    let result = event_loop(&mut term, &mut app, rx);
    ratatui::restore();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("clortho: {e}");
            1
        }
    }
}
