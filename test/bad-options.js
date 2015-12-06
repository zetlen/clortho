'use strict';
const tape = require('tape');
const keytar = require('keytar');
const clortho = require('../');
const EM = require('../src/error-manager');

const badSuccess = t => x => t.end('should not have worked, but ' + x);

tape('no options', assert => {
  assert.plan(2);
  clortho()
  .then(
    badSuccess,
    e => {
      assert.equal(EM.getCode(e), 'INVALID_OPTS', 'error code correct');
      assert.ok(
        ~e.message.indexOf('No configuration object'),
        'invalid opts message sent: ' + e
      );
    }
  );
});

tape('no service', assert => {
  assert.plan(2);
  clortho({ })
  .then(
    badSuccess,
    e => {
      assert.equal(EM.getCode(e), 'INVALID_OPTS', 'error code correct');
      assert.ok(
        ~e.message.indexOf('No service name'),
        'invalid opts message sent: ' + e
      );
    }
  );
});

tape('no message', assert => {
  assert.plan(2);
  clortho({
    service: 'Gozer'
  })
  .then(
    badSuccess,
    e => {
      assert.equal(EM.getCode(e), 'INVALID_OPTS', 'error code correct');
      assert.ok(
        ~e.message.indexOf('No prompt message'),
        'invalid opts message sent: ' + e
      );
    }
  );
});

tape('no message ok if prompt is false', assert => {
  let account = {
    username: 'ray@ghostbusters.party',
    password: 'pw123'
  };
  assert.plan(1);
  keytar.replacePassword(
    'Gozer',
    account.username,
    account.password
  );
  clortho({
    service: 'Gozer',
    prompt: false,
    username: account.username
  })
  .then(
    cred => assert.deepEqual(cred, account, 'credential wrong ' + cred),
    e => assert.err(e)
  );
});
