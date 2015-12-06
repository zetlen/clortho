'use strict';
const tape = require('tape');
const keytar = require('keytar');
const clortho = require('../');

const shouldNotHaveFailed =
  (t, s) => x => t.end(s + ' failed: ' + x);

const shouldNotHaveWorked =
  (t, s) => x => t.end(s + ' should not have worked: ' + x);

tape('service object getFromKeychain', assert => {
  assert.plan(2);
  let vinz = clortho.forService('Gozer');
  keytar.deletePassword('Gozer', 'vinz@clortho.horse');
  vinz.getFromKeychain('vinz@clortho.horse')
  .then(
    shouldNotHaveWorked(assert, 'getFromKeychain'),
    e => assert.ok(
      ~e.message.indexOf('GET_FAILURE'),
      'get failure message sent: ' + e
    )
  );
  keytar.replacePassword('Gozer', 'vinz@clortho.horse', 'pw123');
  vinz.getFromKeychain('vinz@clortho.horse')
  .then(
    c => assert.deepEqual(
      c,
      { username: 'vinz@clortho.horse', password: 'pw123' },
      'credential received'
    ),
    shouldNotHaveFailed(assert, 'getFromKeychain')
  );
});
