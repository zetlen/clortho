'use strict';

var childProcess = require('child_process');
module.exports = function (s, isFile) {
  return new Promise(function (resolve, reject) {
    var child =  childProcess.exec('powershell -' + (isFile ? 'File' : 'Command') + ' ' + s, { stdio: 'ignore' }, function (e, stdout, stderr) {
      if (e) return reject(e);
      if (stderr) return reject(new Error(stderr));
      resolve(stdout.trim());
    });
    child.stdin.end();
  });
};