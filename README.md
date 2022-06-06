# unitypackage-to-upm

> Convert unitypackage files to upm so you can use the package manager.

## Usage

1. Create a `package.json` file. In most cases, you can use something like this:
   ```json
   {
       "name" : "your.package.name",
       "displayName" : "Your Package Name",
       "version" : "0.1.0",
       "unity" : "2019.4",
       "description" : "A description of your package"
   }
   ```
   If you need more options, for example if there are dependencies, look at the [full `package.json` reference].
2. Execute `unitypackage-to-upm <PACKAGE.UNITYPACKAGE> <PACKAGE.JSON> <UPM.ZIP>`.

The resulting zip file can be extracted somewhere and then added as a user package.

[full `package.json` reference]: https://vcc.docs.vrchat.com/vpm/packages#package-format

## Known issues

- It only works for some unitypackage files. Shader packages are probably okay. Packages that contain scripts are unlikely to work.
- VRChat Creator Companion 0.2.5's UI supports having at most three packages. Additional packages will not be visible or selectable. You can work around this by adding the packages through Unity's package manager, but this is less convenient and can confuse VRChat Creator Companion.
