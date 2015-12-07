'use strict';

var keytar = require('keytar');
var ErrorManager = require('./error-manager');

module.exports = {
  get: function get(service, username) {
    return new Promise(function (y, n) {
      var pw = undefined;
      try {
        pw = keytar.getPassword(service, username);
      } catch (e) {
        return n(ErrorManager.create('KEYCHAIN_FAILURE', e.message));
      }
      if (pw) return y(pw);
      return n(ErrorManager.create('GET_FAILURE', 'No ' + service + ' password found for ' + username + '.'));
    });
  },
  set: function set(service, username, password) {
    return new Promise(function (y, n) {
      var success = undefined;
      try {
        success = keytar.replacePassword(service, username, password);
      } catch (e) {
        return n(ErrorManager.create('KEYCHAIN_FAILURE', e.message));
      }
      if (success) return y(success);
      return n(ErrorManager.create('SAVE_FAILURE', 'Could not save ' + service + ' password for ' + username + ' in keychain.'));
    });
  },
  remove: function remove(service, username) {
    return new Promise(function (y, n) {
      var fail = function fail() {
        return n(ErrorManager.create('DELETE_FAILURE', 'Could not delete ' + service + ' password for ' + username + ' in keychain.'));
      };
      var success = undefined;
      try {
        success = keytar.deletePassword(service, username);
      } catch (e) {
        return fail();
      }
      return success ? y() : fail();
    });
  }
};