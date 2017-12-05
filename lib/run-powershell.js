'use strict';

var childProcess = require('child_process');
module.exports = function (s, isFile) {
  return new Promise(function (y, n) {
    return childProcess.exec('powershell -' + (isFile ? 'File ' : 'Command') + ' ' + (isFile ? '"': '') + s + (isFile ? '"': ''), { stdio: 'ignore' }, function (e, stdout, stderr) {
      if (e) return n(e);
      if (stderr) return n(new Error(stderr));
      y(stdout.trim());
    });
  });
};