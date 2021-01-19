'use strict';

var inquirer = require('inquirer');

module.exports = function (service, username, message) {
  return new Promise(function (resolve, reject) {
    process.stdout.write('\x07\n'); // ding!
    console.log(message || 'Please enter your credentials for ' + service + '.');
    var prompts = [{
      type: 'input',
      name: 'username',
      message: 'Username for ' + service,
      validate: function validate(str) {
        return !!str;
      },
      when: function when() {
        return !username;
      },
      default: username
    }, {
      type: 'password',
      name: 'password',
      message: function message(answers) {
        return 'Password for ' + (answers.username || username);
      },
      validate: function validate(str) {
        return !!str;
      }
    }];
    try {
      inquirer.prompt(prompts, function (a) {
        a.username = a.username || username;
        resolve(a);
      });
    } catch (e) {
      reject(e);
    }
  });
};