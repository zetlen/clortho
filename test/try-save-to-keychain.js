'use strict';
const tape = require('tape');
const clortho = require('../');
const keytar = require('keytar');

tape('try save bad credential to keychain', assert => {
  assert.plan(1);
  keytar.deletePassword('Gozer', 'vinz@clortho.horse');
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

tape('try save good credential to keychain', assert => {
  assert.plan(2);
  keytar.deletePassword('Gozer', 'vinz@clortho.horse');
  let vinz = clortho.forService('Gozer');
  let goodCredential = {
    username: 'vinz@clortho.horse',
    password: 'slor'
  };
  vinz.trySaveToKeychain(goodCredential)
  .then(cred => {
    assert.equal(cred, goodCredential, 'credential passed through');
    let pw = keytar.getPassword('Gozer', goodCredential.username);
    assert.equal(pw, goodCredential.password, 'credential stored');
  }).catch(assert.end);
});
