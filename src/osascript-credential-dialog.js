'use strict';
const childProcess = require('child_process');
const osa = (username, message) => new Promise((y, n) =>
  childProcess.exec(
    `osascript -e 'tell app "System Events" to return display dialog "${message}" with title "Password for ${username}" with icon caution default answer "" buttons { "Cancel", "OK" } default button 2 with hidden answer'`,
    { stdio: 'ignore' },
    (e, stdout, stderr) => {
      if (e) return n(e);
      if (stderr) return n(new Error(stderr));
      y(stdout.trim());
    }
  )
);
let requirePassword = (username, message, repeated) =>
  osa(username, message).then(r => {
    let m = r.match(/text returned:(.*)/);
    if (!m || !m[1]) {
      return requirePassword(username, !repeated ? 'A password is required.\n\n' + message : message, true);
    }
    return m[1];
  });
module.exports = (username, message) =>
  requirePassword(username, message).then(p => ({
    username: username,
    password: p
  }));
