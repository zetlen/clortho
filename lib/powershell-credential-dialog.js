'use strict';

var runPowershell = require('./run-powershell');
var delimiter = '|||||';
module.exports = function (service, username, message) {
  message = message || 'Please enter your username and password to login to ' + service + '.';
  return runPowershell('$c = Get-Credential -Message \\"' + message + '\\" -Username ' + username + '; ' + ('$c.GetNetworkCredential().username; echo \\"' + delimiter + '\\"; ') + '$c.GetNetworkCredential().password;').then(function (r) {
    var answers = r.split(delimiter).map(function (s) {
      return s.trim();
    });
    return {
      username: answers[0],
      password: answers[1]
    };
  });
};