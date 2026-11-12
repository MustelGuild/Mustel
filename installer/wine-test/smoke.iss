[Setup]
AppName=WineTest
AppVersion=1.0
DefaultDirName=C:\WineTest
PrivilegesRequired=lowest
OutputDir=Z:\out
OutputBaseFilename=wine-test-setup
Uninstallable=yes

[Files]
Source: "Z:\test\dummy.txt"; DestDir: "{app}"
