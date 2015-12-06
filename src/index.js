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

let clortho = (opts = {service, username, message, cli, keychain, refresh}) =>
  Promise.resolve().then(() => {
    if (!opts) {
      throw ErrorManager.create(
        'INVALID_OPTS',
        'No configuration object supplied.'
      );
    }
    let vinz = clortho.forService(service);
    if (keychain === false) {
      return vinz.prompt(username, message, cli);
    }
    if (prompt === false) {
      return vinz.getFromKeychain(username);
    }
    if (refresh || !username) {
      return vinz.prompt(username, message, cli)
      .then(vinz.trySaveToKeychain);
    }
    return vinz.getFromKeychain(username)
    .catch(() => null)
    .then(cred => cred || vinz.prompt(username, message, cli))
    .then(vinz.trySaveToKeychain);
  });

clortho.forService = service => {
  if (!service) {
    throw ErrorManager.create(
      'INVALID_OPTS',
      'No service name supplied. Please supply a service name for this credential. It can be arbitrary.'
    );
  }
  return {
    getFromKeychain(username) {
      return keychain.get(service, username);
    },
    prompt(username, message, cli) {
      if (cli === true) {
        return cliPrompt(service, username, message);
      } else if (platform === 'darwin' && username) {
        return osaDialog(service, username, message);
      } else if (platform.indexOf('win') === 0 && hasPowershell) {
        return psDialog(service, username, message);
      } else if (cli === false) {
        return cliPrompt(service, username, message);
      } else {
        return Promise.reject(
          ErrorManager.create(
            'NO_PROMPT_STRATEGY',
            'No acceptable prompting system found.'
          )
        );
      }
    },
    saveToKeychain(username, password) {
      return keychain.set(service, username, password);
    },
    trySaveToKeychain(credential) {
      return keychain.set(service, credential.username, credential.password)
      .catch(() => credential);
    }
  };
};

module.exports = clortho;
