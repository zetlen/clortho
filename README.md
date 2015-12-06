![http://i.imgur.com/RiMmYgU.png](The destructor is coming!)

[![js-semistandard-style](https://img.shields.io/badge/code%20style-semistandard-brightgreen.svg?style=flat-square)](https://github.com/Flet/semistandard)

:key: :ghost: **Clortho** :ghost: :key: is a library for asking for service passwords from node scripts. It is [not](https://github.com/pivotal/vinz-clortho) the [first](https://github.com/mozilla/vinz-clortho) library to be named after Louis Tully, but I chose a new form for him: that of a giant Slor!

#### Use Clortho if:
 - You're using Node as a scripting language with user interaction, such as a Yeoman generator, or a Grunt or Gulp plugin.
 - You need to prompt the user for a password to some external service.
 - You want to prompt the user in a friendly way, using GUI tools where available.
 - You want to store the password securely.
 
*Clortho combines keychain authentication with user prompting, so if user interaction isn't a part of your app--that is, if it's a server--then Clortho is not appropriate. If you just want keychain access, you're looking for [node-keytar][1].*

### Usage

`clortho` is a function that returns a promise for a credential. A credential is an object with `username` and `password` properties. The `password` will be in plaintext, so don't cross the streams with it.

```js
  const clortho = require('clortho');

  clortho({
    service: 'Gozer',
    username: 'vinz@clortho.horse',
    message: 'I am Vinz, Vinz Clortho, Keymaster of Gozer. Volguus Zildrohar, Lord of the Sebouillia. Are you the Gatekeeper?'
  }).then(credential => {
    console.log(credential.username);
    console.log(credential.password);
  });
```

The above is the simplest usage; it should work for most cases, because it does some sane things.

1. First, it checks your operating system's credential store or keychain for a password for the named `service` and `username`, using [node-keytar][1].

2. If it finds a password available in the keychain, it skips prompting the user and resolves the promise with the credential.

3. If it doesn't find a password, it prompts the user with an OS-appropriate authentication prompt. In Windows 7 and above, it looks like this:

  ![Windows PowerShell style](http://i.imgur.com/y79xLc7.png)

  In OSX, it looks like this:

  ![OSX AppleScript style](http://i.imgur.com/YWUxewA.png)

  On other operating systems, or if the username was not supplied on OSX, the prompt occurs in the terminal running the program.

  ![CLI terminal style](http://i.imgur.com/nMnyciR.png)

4. If the user clicks "Cancel", the promise rejects. If the user enters a password, however...

5. It stores the username and password in the system keychain.

6. Then, it fulfills the promise with the credential object.

The available options for the `clortho(opts)` function are:
 - `service`: **Required.** Name of the service for which Clortho is getting a credential. This can be any arbitrary string, like "zuul" or "AWS Sandbox".
 - `username`: **Optional.** The username for which Clortho is getting a password. If this is not supplied, Clortho will ask for both a username and password.
 - `message`: **Optional.** A custom message to display with the password prompt, instead of the default "Please enter your username and password".
 - `cli`: **Optional.** If this is `true`, the prompt step will always use the CLI in-terminal prompt style. If it is `false`, the prompt step will *never* use that style. Default is `undefined`, which will allow Clortho to select an OS-appropriate prompt style.
 - `refresh`: **Optional.** If this is set to `true`, then Clortho will not check the keychain before prompting. This is appropriate to use if the password fails the first time. Default `false`.

### API

The sensible default above doesn't work in every case. Fortunately, the default `clortho` function is composed of several functions that can be exposed as separate steps. For instance, this example:

```js
  const clortho = require('clortho');

  clortho({
    service: 'Gozer',
    username: 'vinz@clortho.horse',
    message: 'I am Vinz, Vinz Clortho, Keymaster of Gozer. Volguus Zildrohar, Lord of the Sebouillia. Are you the Gatekeeper?'
  }).then(credential => {
    console.log(credential.username);
    console.log(credential.password);
  });
```

is equivalent to:

```js
  const vinz = require('clortho').forService('Gozer');
  
  vinz.getFromKeychain('vinz@clortho.horse')
  .catch(() =>
    vinz.prompt(
      'vinz@clortho.horse'
      'I am Vinz, Vinz Clortho, Keymaster of Gozer. Volguus Zildrohar, Lord of the Sebouillia. Are you the Gatekeeper?'
    )
    .then(vinz.trySaveToKeychain)
  );
```

You can obtain a decomposed object like the above, by running `clortho.forService(serviceName)`. It has the following methods, all of which return promises:

##### `getFromKeychain(username)`
Takes a string `username`. Queries the system keychain for the username under the service. Resolves with a credential object. Rejects if a credential is not found or the keychain query failed for another reason.

##### `prompt(username, message, cli)`
Prompts the user with a system-appropriate dialog or prompt. The `username` string is optional (though on OSX, a missing `username` will make the system fall back to CLI style). The `message` string is optional, and works as above in the main `clortho` function. The `cli` boolean is optional. If it is `true`, then the prompt will **always** use the CLI terminal style. If it is `false`, then the prompt will **never** use the CLI terminal style. If it is any other value, or missing, then the prompt will detect the appropriate style to use. Resolves with a credential object. Rejects if the user cancels.

##### `saveToKeychain(username, password)`
Both arguments are required. Saves the password securely to the system keychain. Resolves `true` if save was successful. Rejects if save failed for any reason.

##### `trySaveToKeychain(credetial)`
Instead of separate `username` and `password` arguments like `saveToKeychain`, this method takes a credential object with `username` and `password` properties, and attempts to save it to the system keychain. This method **always resolves with the credential again**. It is meant as a pass-through method that should not notify if it fails.

[1]: https://github.com/atom/node-keytar "node-keytar"
