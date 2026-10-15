using System;
using System.IO;
using System.Text.Json;

using FurLab.Core.Models;
using FurLab.Tests.Support;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class UserConfigServiceTests
{
    private string _testDirectory = null!;
    private Core.Logging.ILogger _logger = null!;

    [TestInitialize]
    public void Setup()
    {
        _testDirectory = Path.Combine(Path.GetTempPath(), $"UserConfigServiceTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDirectory);
        _logger = new TestLogger();
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (Directory.Exists(_testDirectory))
        {
            Directory.Delete(_testDirectory, recursive: true);
        }
    }

    [TestMethod(DisplayName = "LoadConfig creates default file when it does not exist")]
    public void LoadConfig_NoFile_CreatesDefault()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var config = service.LoadConfig();

        Assert.IsNotNull(config);
        Assert.IsNotNull(config.Defaults);
        Assert.AreEqual(0, config.Servers.Count);
        Assert.IsTrue(File.Exists(service.GetConfigFilePath()));
    }

    [TestMethod(DisplayName = "SaveConfig and LoadConfig round-trip preserves data")]
    public void SaveAndLoad_RoundTrip_PreservesData()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var config = new UserConfig
        {
            Servers = [new ServerConfigEntry
            {
                Name = "test-server",
                Host = "localhost",
                Port = 5432,
                Username = "postgres",
                EncryptedPassword = "encrypted-blob",
                Databases = ["db1", "db2"]
            }],
            Defaults = new UserDefaults
            {
                OutputDirectory = "./output",
                MaxParallelism = 8
            }
        };

        service.SaveConfig(config);
        var loaded = service.LoadConfig();

        Assert.AreEqual(1, loaded.Servers.Count);
        Assert.AreEqual("test-server", loaded.Servers[0].Name);
        Assert.AreEqual("localhost", loaded.Servers[0].Host);
        Assert.AreEqual(5432, loaded.Servers[0].Port);
        Assert.AreEqual("postgres", loaded.Servers[0].Username);
        Assert.AreEqual("encrypted-blob", loaded.Servers[0].EncryptedPassword);
        Assert.AreEqual(2, loaded.Servers[0].Databases.Count);
        Assert.AreEqual("./output", loaded.Defaults!.OutputDirectory);
        Assert.AreEqual(8, loaded.Defaults.MaxParallelism);
    }

    [TestMethod(DisplayName = "LoadConfig with JSONC (comments) parses correctly")]
    public void LoadConfig_WithJsoncComments_ParsesCorrectly()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var jsonc = @"{
  // This is a comment
  ""servers"": [
    {
      ""name"": ""dev"", /* inline comment */
      ""host"": ""localhost"",
      ""port"": 5432,
      ""username"": ""postgres""
    }
  ],
  ""defaults"": {
    ""outputDirectory"": ""./results""
  }
}";
        File.WriteAllText(service.GetConfigFilePath(), jsonc);

        var config = service.LoadConfig();

        Assert.AreEqual(1, config.Servers.Count);
        Assert.AreEqual("dev", config.Servers[0].Name);
    }

    [TestMethod(DisplayName = "LoadConfig with invalid JSON throws InvalidOperationException")]
    public void LoadConfig_InvalidJson_ThrowsInvalidOperationException()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        File.WriteAllText(service.GetConfigFilePath(), "{ invalid json }");

        var threwExpected = false;
        try { service.LoadConfig(); }
        catch (InvalidOperationException) { threwExpected = true; }
        catch (JsonException) { threwExpected = true; }
        Assert.IsTrue(threwExpected);
    }

    [TestMethod(DisplayName = "AddOrUpdateServer adds new server")]
    public void AddOrUpdateServer_NewServer_AddsToList()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var server = new ServerConfigEntry
        {
            Name = "new-server",
            Host = "db.example.com",
            Port = 5432,
            Username = "admin"
        };

        service.AddOrUpdateServer(server);
        var servers = service.GetServers();

        Assert.AreEqual(1, servers.Count);
        Assert.AreEqual("new-server", servers[0].Name);
    }

    [TestMethod(DisplayName = "AddOrUpdateServer updates existing server")]
    public void AddOrUpdateServer_ExistingServer_UpdatesInPlace()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        service.AddOrUpdateServer(new ServerConfigEntry { Name = "srv", Host = "old-host", Port = 5432, Username = "user" });
        service.AddOrUpdateServer(new ServerConfigEntry { Name = "srv", Host = "new-host", Port = 5433, Username = "admin" });

        var server = service.GetServer("srv");
        Assert.IsNotNull(server);
        Assert.AreEqual("new-host", server.Host);
        Assert.AreEqual(5433, server.Port);
        Assert.AreEqual("admin", server.Username);
    }

    [TestMethod(DisplayName = "RemoveServer removes existing server")]
    public void RemoveServer_ExistingServer_RemovesAndReturnsTrue()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        service.AddOrUpdateServer(new ServerConfigEntry { Name = "srv", Host = "host", Port = 5432, Username = "user" });

        var result = service.RemoveServer("srv");
        Assert.IsTrue(result);
        Assert.AreEqual(0, service.GetServers().Count);
    }

    [TestMethod(DisplayName = "RemoveServer returns false for non-existent server")]
    public void RemoveServer_NonexistentServer_ReturnsFalse()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var result = service.RemoveServer("nonexistent");
        Assert.IsFalse(result);
    }

    [TestMethod(DisplayName = "GetServer returns server by name case-insensitive")]
    public void GetServer_CaseInsensitive_ReturnsServer()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        service.AddOrUpdateServer(new ServerConfigEntry { Name = "MyServer", Host = "host", Port = 5432, Username = "user" });

        var server = service.GetServer("myserver");
        Assert.IsNotNull(server);
        Assert.AreEqual("MyServer", server.Name);
    }

    [TestMethod(DisplayName = "ValidateAndApplyDefaults applies defaults for omitted fields")]
    public void LoadConfig_MissingDefaults_AppliesDefaults()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var json = @"{""servers"": [{""name"": ""srv"",""host"": ""host"",""username"": ""user""}]}";
        File.WriteAllText(service.GetConfigFilePath(), json);

        var config = service.LoadConfig();

        Assert.AreEqual(5432, config.Servers[0].Port);
        Assert.AreEqual("Prefer", config.Servers[0].SslMode);
        Assert.AreEqual(30, config.Servers[0].Timeout);
        Assert.AreEqual(4, config.Servers[0].MaxParallelism);
    }

    [TestMethod(DisplayName = "GetDefaults returns defaults when configured")]
    public void GetDefaults_WhenConfigured_ReturnsValues()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        var config = new UserConfig { Defaults = new UserDefaults { OutputDirectory = "/tmp/out", MaxParallelism = 8 } };
        service.SaveConfig(config);

        var defaults = service.GetDefaults();
        Assert.AreEqual("/tmp/out", defaults.OutputDirectory);
        Assert.AreEqual(8, defaults.MaxParallelism);
    }

    [TestMethod(DisplayName = "ConfigFileExists returns false when file does not exist")]
    public void ConfigFileExists_NoFile_ReturnsFalse()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        Assert.IsFalse(service.ConfigFileExists());
    }

    [TestMethod(DisplayName = "ConfigFileExists returns true after LoadConfig")]
    public void ConfigFileExists_AfterLoad_ReturnsTrue()
    {
        var service = new TestableUserConfigService(_logger, _testDirectory);
        service.LoadConfig();
        Assert.IsTrue(service.ConfigFileExists());
    }
}
