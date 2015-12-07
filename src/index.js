'use strict';
const {
  isOSX,
  isWindows,
  hasPowershell } = require('./os-features');
const psDialog = require('./powershell-credential-dialog');
const osaDialog = require('./osascript-credential-dialog');
const cliPrompt = require('./cli-credential-prompt');
const keychain = require('./keychain-access');
const ErrorManager = require('./error-manager');

let clortho = (opts) =>
  Promise.resolve().then(() => {
    if (!opts) {
      throw ErrorManager.create(
        'INVALID_OPTS',
        'No configuration object supplied.'
      );
    }
    let {service, username, message, cli, refresh} = opts;
    let vinz = clortho.forService(service);
    if (refresh || !username) {
      return vinz.prompt(username, message, cli).then(vinz.trySaveToKeychain);
    }
    return vinz.getFromKeychain(username)
    .catch(() =>
      vinz.prompt(username, message, cli).then(vinz.trySaveToKeychain)
    );
  });

let getOsPrompt = (username, message, cli) => {
  if (isOSX && username) return osaDialog;
  if (isWindows && hasPowershell) return psDialog;
  if (cli !== false) return cliPrompt;
};

clortho.forService = service => {
  if (!service) {
    throw ErrorManager.create(
      'INVALID_OPTS',
      'No service name supplied. Please supply a service name for this credential. It can be arbitrary.'
    );
  }
  if (typeof service !== 'string') {
    throw ErrorManager.create(
      'INVALID_OPTS',
      'Service name must be a string.'
    );
  }
  return {
    getFromKeychain (username) {
      return keychain.get(service, username);
    },
    prompt (username, message, cli) {
      let osPrompt;
      if (cli === true) {
        osPrompt = cliPrompt;
      } else {
        osPrompt = getOsPrompt(username, message, cli);
      }
      if (osPrompt) {
        return osPrompt(service, username, message);
      } else {
        return Promise.reject(
          ErrorManager.create(
            'NO_PROMPT_STRATEGY',
            'No acceptable prompting system found.'
          )
        );
      }
    },
    saveToKeychain (username, password) {
      return keychain.set(service, username, password);
    },
    trySaveToKeychain (credential) {
      return keychain.set(service, credential.username, credential.password)
      .then(
        () => credential,
        () => credential
      );
    },
    removeFromKeychain (username) {
      return keychain.remove(service, username);
    }
  };
};

module.exports = clortho;
