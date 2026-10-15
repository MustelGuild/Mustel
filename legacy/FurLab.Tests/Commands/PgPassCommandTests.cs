using FurLab.CLI;
using FurLab.Core.Models;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class PgPassCommandTests
{
    [TestMethod(DisplayName = "IsValidHost returns true for localhost")]
    public void SecurityUtils_IsValidHost_Localhost_ReturnsTrue()
    {
        var result = SecurityUtils.IsValidHost("localhost");
        Assert.IsTrue(result);
    }

    [TestMethod(DisplayName = "IsValidHost returns true for IP address")]
    public void SecurityUtils_IsValidHost_IpAddress_ReturnsTrue()
    {
        var result = SecurityUtils.IsValidHost("192.168.1.1");
        Assert.IsTrue(result);
    }

    [TestMethod(DisplayName = "IsValidHost returns false for empty string")]
    public void SecurityUtils_IsValidHost_Empty_ReturnsFalse()
    {
        var result = SecurityUtils.IsValidHost("");
        Assert.IsFalse(result);
    }

    [TestMethod(DisplayName = "IsValidHost returns false for host with spaces")]
    public void SecurityUtils_IsValidHost_Spaces_ReturnsFalse()
    {
        var result = SecurityUtils.IsValidHost("host with spaces");
        Assert.IsFalse(result);
    }

    [TestMethod(DisplayName = "IsValidPort returns true for valid port 5432")]
    public void SecurityUtils_IsValidPort_Valid5432_ReturnsTrue()
    {
        var result = SecurityUtils.IsValidPort("5432");
        Assert.IsTrue(result);
    }

    [TestMethod(DisplayName = "IsValidPort returns true for valid port 1")]
    public void SecurityUtils_IsValidPort_Valid1_ReturnsTrue()
    {
        var result = SecurityUtils.IsValidPort("1");
        Assert.IsTrue(result);
    }

    [TestMethod(DisplayName = "IsValidPort returns true for valid port 65535")]
    public void SecurityUtils_IsValidPort_Valid65535_ReturnsTrue()
    {
        var result = SecurityUtils.IsValidPort("65535");
        Assert.IsTrue(result);
    }

    [TestMethod(DisplayName = "IsValidPort returns false for port 0")]
    public void SecurityUtils_IsValidPort_Zero_ReturnsFalse()
    {
        var result = SecurityUtils.IsValidPort("0");
        Assert.IsFalse(result);
    }

    [TestMethod(DisplayName = "IsValidPort returns false for port 65536")]
    public void SecurityUtils_IsValidPort_TooHigh_ReturnsFalse()
    {
        var result = SecurityUtils.IsValidPort("65536");
        Assert.IsFalse(result);
    }

    [TestMethod(DisplayName = "IsValidPort returns false for non-numeric")]
    public void SecurityUtils_IsValidPort_NonNumeric_ReturnsFalse()
    {
        var result = SecurityUtils.IsValidPort("abc");
        Assert.IsFalse(result);
    }

    [TestMethod]
    public void PgPassEntry_CreatedWithCorrectValues()
    {
        var entry = new PgPassEntry
        {
            Hostname = "localhost",
            Port = "5432",
            Database = "mydb",
            Username = "postgres",
            Password = "secret"
        };

        Assert.AreEqual("localhost", entry.Hostname);
        Assert.AreEqual("5432", entry.Port);
        Assert.AreEqual("mydb", entry.Database);
        Assert.AreEqual("postgres", entry.Username);
        Assert.AreEqual("secret", entry.Password);
    }
}
