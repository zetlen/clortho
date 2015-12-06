const tape = require('tape');
const clortho = require('../');
const keytar = require('keytar');

tape('keychain only', assert => {
  assert.plan(1);
  keytar.deletePassword('Gozer', 'vinz@clortho.horse');
  clortho({
    service: 'Gozer',
    username: 'vinz@clortho.horse',
    message: 'You will perish in flames!',
    prompt: false
  }).then(
    cred => assert.fail('should not have worked, but did with ' + cred),
    fail => {
      assert.ok(
        ~fail.message.indexOf('GET_FAILURE'),
        'get failure message sent: ' + fail
      );
    }
  );
});
