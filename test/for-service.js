'use strict';
const tape = require('tape');
const clortho = require('../');

tape('try create forService with no or bad service', assert => {
  assert.plan(3);
  assert.throws(
    () => clortho.forService(),
    /No service name supplied/,
    'errors on no service'
  );
  assert.throws(
    () => clortho.forService(''),
    /No service name supplied/,
    'errors on empty service name'
  );
  assert.throws(
    () => clortho.forService(5),
    /Service name must be a string/,
    'errors on non-string service'
  );
});

const methods = [
  'getFromKeychain',
  'prompt',
  'saveToKeychain',
  'trySaveToKeychain'
];
tape('forService returns an object with methods', assert => {
  assert.plan(methods.length);
  let vinz = clortho.forService('Gozer');
  methods.forEach(method =>
    assert.equal(
      'function',
      typeof vinz[method],
      method + ' exists'
    )
  );
});
