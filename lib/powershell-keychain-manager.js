'use strict';

var path = require('path');
var jsStringEscape = require('js-string-escape');
var runPowershell = require('./run-powershell');
var ErrorManager = require('./error-manager');
var credManPath = path.resolve(__dirname, '../CredMan.ps1');
var runCredMan = function runCredMan(cmd, opts) {
  return runPowershell(Object.keys(opts).reduce(function (cmd, k) {
    return cmd + (' -' + k + ' \'' + jsStringEscape(opts[k]) + '\'');
  }, credManPath + ' -' + cmd), true);
};
var passwordLineRE = /^[\s\t]*Password[\s\t]*:[\s\t]?'(.*)'/;
var createTargetName = function createTargetName(service, account) {
  return service + ';user=' + account;
};

module.exports = {
  get: function get(service, account) {
    return runCredMan('GetCred', {
      Target: createTargetName(service, account)
    }).then(function (res) {
      if (res.match(/was not found\.$/)) {
        throw ErrorManager.create('GET_FAILURE', 'Could not find ' + service + ' password for ' + account);
      }
      var pwl = res.split('\n').find(function (l) {
        return !!l.match(passwordLineRE);
      });
      if (!pwl) {
        throw ErrorManager.create('GET_FAILURE', 'Unknown error finding ' + service + ' password for ' + account + '.');
      }
      return { username: account, password: pwl.match(passwordLineRE)[1] };
    });
  },
  set: function set(service, account, password) {
    return runCredMan('AddCred', {
      Target: createTargetName(service, account),
      User: account,
      Pass: password
    }).then(function (res) {
      if (res.indexOf('Successfully') !== 0) {
        throw ErrorManager.create('KEYCHAIN_FAILURE', 'Unknown error saving to keychain');
      }
      return { username: account, password: password };
    });
  },
  remove: function remove(service, account) {
    return runCredMan('DelCred', {
      Target: createTargetName(service, account)
    }).then(function (res) {
      if (res.indexOf('Successfully') !== 0) {
        throw ErrorManager.create('KEYCHAIN_FAILURE', 'Unknown error removing from keychain');
      }
    });
  }
};