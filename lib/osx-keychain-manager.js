'use strict';

var keychain = require('keychain');
var ErrorManager = require('./error-manager');

module.exports = {
  get: function get(service, account) {
    return new Promise(function (y, n) {
      keychain.getPassword({ service: service, account: account }, function (err, password) {
        if (err) {
          return n(ErrorManager.create('GET_FAILURE', err.message));
        }
        return y({ username: account, password: password });
      });
    });
  },
  set: function set(service, account, password) {
    return new Promise(function (y, n) {
      keychain.setPassword({ service: service, account: account, password: password }, function (err) {
        if (err) {
          return n(ErrorManager.create('KEYCHAIN_FAILURE', err.message));
        }
        return y({ username: account, password: password });
      });
    });
  },
  remove: function remove(service, account) {
    return new Promise(function (y, n) {
      keychain.deletePassword({ service: service, account: account }, function (err) {
        if (err) {
          return n(ErrorManager.create('KEYCHAIN_FAILURE', err.message));
        }
        return y();
      });
    });
  }
};