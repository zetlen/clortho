const keychain = require('keychain');
const ErrorManager = require('./error-manager');

module.exports = {
  get: (service, account) => new Promise((y, n) => {
    keychain.getPassword({ service, account }, (err, password) => {
      if (err) {
        return n(
          ErrorManager.create(
            'GET_FAILURE',
            err.message
          )
        );
      }
      return y({ username: account, password });
    });
  }),
  set: (service, account, password) => new Promise((y, n) => {
    keychain.setPassword({ service, account, password }, err => {
      if (err) {
        return n(
          ErrorManager.create(
            'KEYCHAIN_FAILURE',
            err.message
          )
        );
      }
      return y({ username: account, password });
    });
  }),
  remove: (service, account) => new Promise((y, n) => {
    keychain.deletePassword({ service, account }, err => {
      if (err) {
        return n(
          ErrorManager.create(
            'KEYCHAIN_FAILURE',
            err.message
          )
        );
      }
      return y();
    });
  })
};
