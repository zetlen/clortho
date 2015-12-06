'use strict';

var os = require('os');
var which = require('which');
var platform = process.env.TEST_PLAT || os.platform();
var psDialog = require('./powershell-credential-dialog');
var osaDialog = require('./osascript-credential-dialog');
var cliPrompt = require('./cli-credential-prompt');
var keychain = require('./keychain-access');
var ErrorManager = require('./error-manager');
var hasPowershell = undefined;
try {
  hasPowershell = which.sync('powershell');
} catch (e) {
  hasPowershell = false;
}

var clortho = function clortho(opts) {
  return Promise.resolve().then(function () {
    if (!opts) {
      throw ErrorManager.create('INVALID_OPTS', 'No configuration object supplied.');
    }
    var service = opts.service;
    var username = opts.username;
    var message = opts.message;
    var cli = opts.cli;
    var refresh = opts.refresh;

    var vinz = clortho.forService(service);
    if (refresh || !username) {
      return vinz.prompt(username, message, cli).then(vinz.trySaveToKeychain);
    }
    return vinz.getFromKeychain(username).catch(function () {
      return vinz.prompt(username, message, cli).then(vinz.trySaveToKeychain);
    });
  });
};

clortho.forService = function (service) {
  if (!service) {
    throw ErrorManager.create('INVALID_OPTS', 'No service name supplied. Please supply a service name for this credential. It can be arbitrary.');
  }
  if (typeof service !== 'string') {
    throw ErrorManager.create('INVALID_OPTS', 'Service name must be a string.');
  }
  return {
    getFromKeychain: function getFromKeychain(username) {
      return keychain.get(service, username).then(function (password) {
        return { username: username, password: password };
      });
    },
    prompt: function prompt(username, message, cli) {
      if (cli === true) {
        return cliPrompt(service, username, message);
      } else if (platform === 'darwin' && username) {
        return osaDialog(service, username, message);
      } else if (platform.indexOf('win') === 0 && hasPowershell) {
        return psDialog(service, username, message);
      } else if (cli !== false) {
        return cliPrompt(service, username, message);
      } else {
        return Promise.reject(ErrorManager.create('NO_PROMPT_STRATEGY', 'No acceptable prompting system found.'));
      }
    },
    saveToKeychain: function saveToKeychain(username, password) {
      return keychain.set(service, username, password);
    },
    trySaveToKeychain: function trySaveToKeychain(credential) {
      return keychain.set(service, credential.username, credential.password).then(function () {
        return credential;
      }, function () {
        return credential;
      });
    }
  };
};

module.exports = clortho;