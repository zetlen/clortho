'use strict';
const os = require('os');
const which = require('which');
const platform = process.env.TEST_PLAT || os.platform();
const psDialog = require('./powershell-credential-dialog');
const osaDialog = require('./osascript-credential-dialog');
const cliPrompt = require('./cli-credential-prompt');
const keychain = require('./keychain-access');
const ErrorManager = require('./error-manager');
let hasPowershell;
try {
  hasPowershell = which.sync('powershell');
} catch (e) {
  hasPowershell = false;
}

let validateOpts = opts => {
  if (!opts) return 'No configuration object supplied.';
  if (!opts.service) return 'No service name supplied. Please supply a service name for this credential. It can be arbitrary.';
  if (opts.prompt !== false && !opts.message) {
    return 'No prompt message supplied. What is this credential for?';
  }
};

module.exports = opts =>
  new Promise((y, n) => {
    let invalid = validateOpts(opts);
    if (invalid) return n(ErrorManager.create('INVALID_OPTS', invalid));

    if (opts.keychain === false) {
      // skip right to prompts then
      return y(null);
    } else {
      return keychain.get(opts.service, opts.username)
      .catch(e => {
        if (
          ErrorManager.getCode(e) === 'GET_FAILURE' &&
          opts.prompt !== false
        ) {
          // that's ok, we'll prompt for it!
          return null;
        }
        // otherwise pass it along
        throw e;
      }).then(y, n);
    }
  })
  .then(password => {
    if (password) {
      return {
        username: opts.username,
        password: password
      };
    }
    if (opts.forceCli) {
      return cliPrompt(opts.username, opts.message);
    } else if (platform === 'darwin' && opts.username) {
      return osaDialog(opts.username, opts.message);
    } else if (platform.indexOf('win') === 0 && hasPowershell) {
      return psDialog(opts.username, opts.message);
    } else if (opts.cli !== false) {
      return cliPrompt(opts.username, opts.message);
    } else {
      throw ErrorManager.create(
        'NO_PROMPT_STRATEGY',
        'No acceptable prompting system found.'
      );
    }
  })
  .then(credential => {
    if (opts.keychain === false) {
      return credential;
    } else {
      return keychain.set(
        opts.service,
        credential.username,
        credential.password
      ).then(() => credential);
    }
  });
