'use strict';
const keytar = require('keytar');
const ErrorManager = require('./error-manager');

module.exports = {
  get: (service, username) => new Promise((y, n) => {
    let pw;
    try {
      pw = keytar.getPassword(service, username);
    } catch (e) {
      return n(
        ErrorManager.create(
          'KEYCHAIN_FAILURE',
          e.message
        )
      );
    }
    if (pw) return y(pw);
    return n(
      ErrorManager.create(
        'GET_FAILURE',
        `No ${service} password found for ${username}.`
      )
    );
  }),
  set: (service, username, password) => new Promise((y, n) => {
    let success;
    try {
      success = keytar.replacePassword(service, username, password);
    } catch (e) {
      return n(
        ErrorManager.create(
          'KEYCHAIN_FAILURE',
          e.message
        )
      );
    }
    if (success) return y(success);
    return n(
      ErrorManager.create(
        'SAVE_FAILURE',
        `Could not save ${service} password for ${username} in keychain.`
      )
    );
  })
};
