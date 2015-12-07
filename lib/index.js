'use strict';

var _require = require('./os-features');

var isOSX = _require.isOSX;
var isWindows = _require.isWindows;
var hasPowershell = _require.hasPowershell;

var psDialog = require('./powershell-credential-dialog');
var osaDialog = require('./osascript-credential-dialog');
var cliPrompt = require('./cli-credential-prompt');
var keychain = require('./keychain-access');
var ErrorManager = require('./error-manager');

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

var getOsPrompt = function getOsPrompt(username, message, cli) {
  if (isOSX && username) return osaDialog;
  if (isWindows && hasPowershell) return psDialog;
  if (cli !== false) return cliPrompt;
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
      return keychain.get(service, username);
    },
    prompt: function prompt(username, message, cli) {
      var osPrompt = undefined;
      if (cli === true) {
        osPrompt = cliPrompt;
      } else {
        osPrompt = getOsPrompt(username, message, cli);
      }
      if (osPrompt) {
        return osPrompt(service, username, message);
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
    },
    removeFromKeychain: function removeFromKeychain(username) {
      return keychain.remove(service, username);
    }
  };
};

module.exports = clortho;