'use strict';
const inquirer = require('inquirer');
const ErrorManager = require('./error-manager');

module.exports = (service, username, message) =>
  new Promise((y, n) => {
    process.stdout.write('\u0007\n'); // ding!
    console.log(
      message || `Please enter your credentials for ${service}.`
    );
    let prompts = [
      {
        type: 'input',
        name: 'username',
        message: `Username for ${service}`,
        validate: str => !!str,
        when: () => !username,
        default: username
      },
      {
        type: 'password',
        name: 'password',
        message: answers =>
          `Password for ${answers.username || username}`,
        validate: str => !!str
      }
    ];
    try {
      inquirer.prompt(prompts, a => {
        a.username = a.username || username;
        y(a);
      });
    } catch (e) { n(e); }
  });
