'use strict';
const tape = require('tape');
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
