using FurLab.CLI;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

/// <summary>
/// Unit tests for SecurityUtils — host, port, and user validations,
/// including pgpass wildcard (*) support.
/// </summary>
[TestClass]
public class SecurityUtilsTests
{
    // =========================================================================
    // IsValidHost — wildcard
    // =========================================================================

    [TestMethod(DisplayName = "IsValidHost with wildcard '*' should return true")]
    [Description("Verifies that the wildcard character '*' is accepted as a valid host in pgpass format.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidHost_Wildcard_ReturnsTrue()
    {
        Assert.IsTrue(SecurityUtils.IsValidHost("*"));
    }

    [TestMethod(DisplayName = "IsValidHost with wildcard and spaces should return false")]
    [Description("Verifies that spaces around the wildcard '*' make the host invalid.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidHost_WildcardWithSpaces_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidHost(" * "));
    }

    [TestMethod(DisplayName = "IsValidHost with wildcard and trailing space should return false")]
    [Description("Verifies that space after the wildcard '*' makes the host invalid.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidHost_WildcardWithTrailingSpace_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidHost("* "));
    }

    // =========================================================================
    // IsValidPort — wildcard
    // =========================================================================

    [TestMethod(DisplayName = "IsValidPort with wildcard '*' should return true")]
    [Description("Verifies that the wildcard character '*' is accepted as a valid port in pgpass format.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidPort_Wildcard_ReturnsTrue()
    {
        Assert.IsTrue(SecurityUtils.IsValidPort("*"));
    }

    [TestMethod(DisplayName = "IsValidPort with wildcard and spaces should return false")]
    [Description("Verifies that spaces around the wildcard '*' in the port make the value invalid.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidPort_WildcardWithSpaces_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidPort(" * "));
    }

    [TestMethod(DisplayName = "IsValidPort with valid numeric value should return true")]
    [Description("Verifies that port numbers within the valid range (1-65535) are accepted.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidPort_ValidNumericPort_ReturnsTrue()
    {
        Assert.IsTrue(SecurityUtils.IsValidPort("5432"));
    }

    [TestMethod(DisplayName = "IsValidPort with negative value should return false")]
    [Description("Verifies that negative port numbers are rejected.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidPort_NegativePort_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidPort("-1"));
    }

    [TestMethod(DisplayName = "IsValidPort with value above maximum should return false")]
    [Description("Verifies that port numbers above 65535 are rejected.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidPort_PortAboveMaximum_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidPort("99999"));
    }

    // =========================================================================
    // IsValidUsername — wildcard
    // =========================================================================

    [TestMethod(DisplayName = "IsValidUsername with wildcard '*' should return true")]
    [Description("Verifies that the wildcard character '*' is accepted as a valid username in pgpass format.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidUsername_Wildcard_ReturnsTrue()
    {
        Assert.IsTrue(SecurityUtils.IsValidUsername("*"));
    }

    [TestMethod(DisplayName = "IsValidUsername with wildcard and spaces should return false")]
    [Description("Verifies that spaces around the wildcard '*' make the username invalid.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidUsername_WildcardWithSpaces_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidUsername(" * "));
    }

    [TestMethod(DisplayName = "IsValidUsername with wildcard and trailing space should return false")]
    [Description("Verifies that space after the wildcard '*' makes the username invalid.")]
    [TestCategory("pgpass-wildcard")]
    public void IsValidUsername_WildcardWithTrailingSpace_ReturnsFalse()
    {
        Assert.IsFalse(SecurityUtils.IsValidUsername("* "));
    }
}
