'use strict';

var ErrorManager = require('./error-manager');
var cache = {};
module.exports = {
  get: function get(service, username) {
    if (cache[service] && cache[service][username]) {
      return Promise.resolve({
        username: username,
        password: cache[service][username]
      });
    }
    return Promise.reject(ErrorManager.create('GET_FAILURE', 'Could not find ' + service + ' password for ' + username + '.'));
  },
  set: function set(service, username, password) {
    cache[service] = cache[service] || {};
    cache[service][username] = password;
    return Promise.resolve({ username: username, password: password });
  },
  remove: function remove(service, username) {
    cache[service] = cache[service] || {};
    delete cache[service][username];
    return Promise.resolve();
  }
};