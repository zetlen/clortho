'use strict';

var childProcess = require('child_process');
var osa = function osa(service, username, message) {
  return new Promise(function (resolve, reject) {
    message = message || 'Please enter your password for ' + username + ' to login to ' + service + '.';
    childProcess.exec('osascript -e \'tell app "System Events" to return display dialog "' + message + '" with title "Password for ' + username + ' at ' + service + '" with icon caution default answer "" buttons { "Cancel", "OK" } default button 2 with hidden answer\'', { stdio: 'ignore' }, function (e, stdout, stderr) {
      if (e) return reject(e);
      if (stderr) return reject(new Error(stderr));
      resolve(stdout.trim());
    });
  });
};
var requirePassword = function requirePassword(service, username, message, repeated) {
  return osa(service, username, message).then(function (r) {
    var m = r.match(/text returned:(.*)/);
    if (!m || !m[1]) {
      return requirePassword(service, username, !repeated ? 'A password is required.\n\n' + message : message, true);
    }
    return m[1];
  });
};
module.exports = function (service, username, message) {
  return requirePassword(service, username, message).then(function (p) {
    return {
      username: username,
      password: p
    };
  });
};