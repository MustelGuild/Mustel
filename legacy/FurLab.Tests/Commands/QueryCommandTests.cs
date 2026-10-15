using System;
using System.IO;

using FurLab.CLI.Commands.Query.Run;
using FurLab.Core.Interfaces;
using FurLab.Core.Services;

using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class QueryCommandTests
{
    private string _testDirectory = null!;
    private string _sqlInputFile = null!;

    [TestInitialize]
    public void Setup()
    {
        _testDirectory = Path.Combine(Path.GetTempPath(), $"QueryCommandTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDirectory);

        _sqlInputFile = Path.Combine(_testDirectory, "test.sql");
        File.WriteAllText(_sqlInputFile, "SELECT * FROM users;");
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (Directory.Exists(_testDirectory))
        {
            Directory.Delete(_testDirectory, recursive: true);
        }
    }

    [TestMethod]
    public void IDatabaseService_CanBeResolvedFromDI()
    {
        var services = new ServiceCollection();
        services.AddFurLabServices();
        services.AddLogging(builder => builder.AddDebug());

        var serviceProvider = services.BuildServiceProvider();

        var databaseService = serviceProvider.GetService<IDatabaseService>();
        Assert.IsNotNull(databaseService);
    }

    // --- UnescapeInlineQuery edge cases ---

    [TestMethod(DisplayName = "UnescapeInlineQuery strips outer double quotes")]
    public void UnescapeInlineQuery_OuterDoubleQuotes_Stripped()
    {
        var result = QueryRunCommand.UnescapeInlineQuery("\"SELECT 1\"");
        Assert.AreEqual("SELECT 1", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery strips outer single quotes")]
    public void UnescapeInlineQuery_OuterSingleQuotes_Stripped()
    {
        var result = QueryRunCommand.UnescapeInlineQuery("'SELECT 1'");
        Assert.AreEqual("SELECT 1", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery with no outer quotes returns unchanged")]
    public void UnescapeInlineQuery_NoOuterQuotes_ReturnsUnchanged()
    {
        var result = QueryRunCommand.UnescapeInlineQuery("SELECT 1");
        Assert.AreEqual("SELECT 1", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery unescapes backslash-escaped double quotes")]
    public void UnescapeInlineQuery_BackslashEscapedDoubleQuotes_Unescaped()
    {
        var result = QueryRunCommand.UnescapeInlineQuery(@"SELECT \""name\"" FROM t");
        Assert.AreEqual("SELECT \"name\" FROM t", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery unescapes backslash-escaped single quotes")]
    public void UnescapeInlineQuery_BackslashEscapedSingleQuotes_Unescaped()
    {
        var result = QueryRunCommand.UnescapeInlineQuery(@"SELECT \''val\'' FROM t");
        Assert.AreEqual("SELECT ''val'' FROM t", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery with mismatched quotes returns unchanged")]
    public void UnescapeInlineQuery_MismatchedQuotes_ReturnsUnchanged()
    {
        var result = QueryRunCommand.UnescapeInlineQuery("\"SELECT 1'");
        Assert.AreEqual("\"SELECT 1'", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery on empty string returns empty")]
    public void UnescapeInlineQuery_EmptyString_ReturnsEmpty()
    {
        var result = QueryRunCommand.UnescapeInlineQuery(string.Empty);
        Assert.AreEqual(string.Empty, result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery on single character in double quotes returns char")]
    public void UnescapeInlineQuery_SingleCharInDoubleQuotes_ReturnsChar()
    {
        var result = QueryRunCommand.UnescapeInlineQuery("\"x\"");
        Assert.AreEqual("x", result);
    }

    [TestMethod(DisplayName = "UnescapeInlineQuery on two double-quote chars returns empty")]
    public void UnescapeInlineQuery_TwoDoubleQuoteChars_ReturnsEmpty()
    {
        var result = QueryRunCommand.UnescapeInlineQuery("\"\"");
        Assert.AreEqual(string.Empty, result);
    }
}
