'use strict';

var codeRe = /clortho: ([A-Z_]+):/;
module.exports = {
  create: function create(code, message) {
    return new Error('clortho: ' + code + ': ' + message);
  },
  getCode: function getCode(err) {
    var m = err.message.match(codeRe);
    return m && m[1];
  }
};