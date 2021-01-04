'use strict';
const tape = require('tape');
const clortho = require('../');
const keychain = require('../lib/keychain-access');

const shouldNotHaveFailed =
  (t, s) => x => t.end(s + ' failed: ' + x);

const shouldNotHaveWorked =
  (t, s) => x => t.end(s + ' should not have worked: ' + x);

tape('service object getFromKeychain', assert => {
  assert.plan(2);
  const vinz = clortho.forService('Gozer');
  keychain.remove('Gozer', 'vinz@clortho.horse')
    .catch(() => null)
    .then(() => {
      vinz.getFromKeychain('vinz@clortho.horse')
        .then(
          shouldNotHaveWorked(assert, 'getFromKeychain'),
          e => assert.ok(
            ~e.message.indexOf('GET_FAILURE'),
            'get failure message sent: ' + e
          )
        );
    })
    .then(() => keychain.set('Gozer', 'vinz@clortho.horse', 'pw123'))
    .then(() => {
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
});
