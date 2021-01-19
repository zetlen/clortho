const keychain = require('keychain');
const ErrorManager = require('./error-manager');

module.exports = {
  get: (service, account) => new Promise((resolve, reject) => {
    keychain.getPassword({ service, account }, (err, password) => {
      if (err) {
        return reject(
          ErrorManager.create(
            'GET_FAILURE',
            err.message
          )
        );
      }
      return resolve({ username: account, password });
    });
  }),
  set: (service, account, password) => new Promise((resolve, reject) => {
    keychain.setPassword({ service, account, password }, err => {
      if (err) {
        return reject(
          ErrorManager.create(
            'KEYCHAIN_FAILURE',
            err.message
          )
        );
      }
      return resolve({ username: account, password });
    });
  }),
  remove: (service, account) => new Promise((resolve, reject) => {
    keychain.deletePassword({ service, account }, err => {
      if (err) {
        return reject(
          ErrorManager.create(
            'KEYCHAIN_FAILURE',
            err.message
          )
        );
      }
      return resolve();
    });
  })
};
