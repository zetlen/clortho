'use strict';
const tape = require('tape');
const clortho = require('../');
const keychain = require('../lib/keychain-access');

tape('try save bad credential to keychain', assert => {
  assert.plan(1);
  keychain.remove('Gozer', 'vinz@clortho.horse')
  .catch(() => null)
  .then(() => {
    let vinz = clortho.forService('Gozer');
    let badCredential = {
      username: {},
      password: ''
    };
    vinz.trySaveToKeychain(badCredential)
    .then(cred => {
      assert.equal(cred, badCredential, 'credential passed through');
    }).catch(assert.end);
  });
});

tape('try save good credential to keychain', assert => {
  assert.plan(2);
  keychain.remove('Gozer', 'vinz@clortho.horse')
  .catch(() => null)
  .then(() => {
    let vinz = clortho.forService('Gozer');
    let goodCredential = {
      username: 'vinz@clortho.horse',
      password: 'slor'
    };
    vinz.trySaveToKeychain(goodCredential)
    .then(cred => {
      assert.equal(cred, goodCredential, 'credential passed through');
      return keychain.get('Gozer', goodCredential.username)
      .then(c => assert.deepEqual(c, goodCredential, 'credential stored'));
    }).catch(assert.end);
  });
});
