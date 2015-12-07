const ErrorManager = require('./error-manager');
let cache = {};
module.exports = {
  get (service, username) {
    if (cache[service] && cache[service][username]) {
      return Promise.resolve({
        username,
        password: cache[service][username]
      });
    }
    return Promise.reject(
      ErrorManager.create(
        'GET_FAILURE',
        `Could not find ${service} password for ${username}.`
      )
    );
  },
  set (service, username, password) {
    cache[service] = cache[service] || {};
    cache[service][username] = password;
    return Promise.resolve({ username, password });
  },
  remove (service, username) {
    cache[service] = cache[service] || {};
    delete cache[service][username];
    return Promise.resolve();
  }
};
