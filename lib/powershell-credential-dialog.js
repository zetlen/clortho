'use strict';

var childProcess = require('child_process');
var ps = function ps(s) {
  return new Promise(function (y, n) {
    return childProcess.exec('powershell /c ' + s, { stdio: 'ignore' }, function (e, stdout, stderr) {
      if (e) return n(e);
      if (stderr) return n(new Error(stderr));
      y(stdout.trim());
    });
  });
};
var delimiter = '|||||';
module.exports = function (service, username, message) {
  message = message || 'Please enter your username and password to login to ' + service + '.';
  return ps('$c = Get-Credential -Message \\"' + message + '\\" -Username ' + username + '; ' + ('$c.GetNetworkCredential().username; echo \\"' + delimiter + '\\"; ') + '$c.GetNetworkCredential().password;').then(function (r) {
    var answers = r.split(delimiter).map(function (s) {
      return s.trim();
    });
    return {
      username: answers[0],
      password: answers[1]
    };
  });
};