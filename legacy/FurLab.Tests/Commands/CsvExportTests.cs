#pragma warning disable IDE0005
using System;
using System.Collections.Generic;
using System.Linq;
using CsvHelper;
using FurLab.CLI.Commands.Query;
#pragma warning restore IDE0005

using Microsoft.VisualStudio.TestTools.UnitTesting;

using IO = System.IO.Path;
using IOFile = System.IO.File;
using IODir = System.IO.Directory;

namespace FurLab.Tests.Commands;

[TestClass]
public class CsvExportTests
{
    private readonly CsvExporter _exporter = new();
    private string _testDirectory = null!;

    [TestInitialize]
    public void Setup()
    {
        _testDirectory = IO.Combine(IO.GetTempPath(), $"CsvExportTests_{Guid.NewGuid():N}");
        IODir.CreateDirectory(_testDirectory);
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (IODir.Exists(_testDirectory))
        {
            IODir.Delete(_testDirectory, recursive: true);
        }
    }

    private static CsvRow CreateSuccessRow(string server, string database, List<string> columns, List<Dictionary<string, string>> data)
    {
        return new CsvRow(server, database, DateTime.UtcNow, "Success", data.Count, string.Empty, 0, columns, data);
    }

    private static List<CsvRow> CreateSuccessResults()
    {
        return
        [
            CreateSuccessRow("dev", "db1", ["id", "name"],
            [
                new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" },
                new Dictionary<string, string> { ["id"] = "2", ["name"] = "Bob" }
            ]),
            CreateSuccessRow("dev2", "db1", ["id", "name"],
            [
                new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" }
            ])
        ];
    }

    [TestMethod(DisplayName = "Consolidated CSV has Server, Database, <query cols> header")]
    public void ConsolidatedCsv_HasCorrectHeader()
    {
        var results = CreateSuccessResults();
        var outputPath = IO.Combine(_testDirectory, "test.csv");

        _exporter.WriteConsolidatedCsv(outputPath, results);

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual("Server,Database,id,name", lines[0]);
    }

    [TestMethod(DisplayName = "Consolidated CSV has one row per query result with Server and Database")]
    public void ConsolidatedCsv_HasDataRowsWithServerAndDatabase()
    {
        var results = CreateSuccessResults();
        var outputPath = IO.Combine(_testDirectory, "test.csv");

        _exporter.WriteConsolidatedCsv(outputPath, results);

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual(4, lines.Length);
        Assert.IsTrue(lines[1].StartsWith("dev,db1,"));
        Assert.IsTrue(lines[2].StartsWith("dev,db1,"));
        Assert.IsTrue(lines[3].StartsWith("dev2,db1,"));
    }

    [TestMethod(DisplayName = "Consolidated CSV does not include execution metadata columns")]
    public void ConsolidatedCsv_DoesNotIncludeExecutionMetadata()
    {
        var results = CreateSuccessResults();
        var outputPath = IO.Combine(_testDirectory, "test.csv");

        _exporter.WriteConsolidatedCsv(outputPath, results);

        var header = IOFile.ReadAllLines(outputPath)[0];
        Assert.IsFalse(header.Contains("ExecutedAt"));
        Assert.IsFalse(header.Contains("Status"));
        Assert.IsFalse(header.Contains("RowCount"));
        Assert.IsFalse(header.Contains("Error"));
    }

    [TestMethod(DisplayName = "BuildColumnList returns columns in first appearance order")]
    public void BuildColumnList_ReturnsColumnsInFirstAppearanceOrder()
    {
        var results = new List<CsvRow>
        {
            new("s1", "db1", DateTime.UtcNow, "Success", 1, string.Empty, 0, ["b", "a"], []),
            new("s1", "db2", DateTime.UtcNow, "Success", 1, string.Empty, 0, ["a", "c"], []),
        };

        var columns = _exporter.BuildColumnList(results);

        CollectionAssert.AreEqual(new[] { "b", "a", "c" }, columns);
    }

    [TestMethod(DisplayName = "BuildColumnList deduplicates columns across result sets")]
    public void BuildColumnList_DeduplicatesColumns()
    {
        var results = new List<CsvRow>
        {
            new("s1", "db1", DateTime.UtcNow, "Success", 1, string.Empty, 0, ["id", "name"], []),
            new("s1", "db2", DateTime.UtcNow, "Success", 1, string.Empty, 0, ["id", "email"], []),
        };

        var columns = _exporter.BuildColumnList(results);

        CollectionAssert.AreEqual(new[] { "id", "name", "email" }, columns);
    }

    [TestMethod(DisplayName = "Consolidated CSV fills missing column with empty string")]
    public void ConsolidatedCsv_MissingColumnFilledWithEmpty()
    {
        var results = new List<CsvRow>
        {
            new("s1", "db1", DateTime.UtcNow, "Success", 1, string.Empty, 0, ["id", "name"],
            [new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" }]),
            new("s1", "db2", DateTime.UtcNow, "Success", 1, string.Empty, 0, ["id"],
            [new Dictionary<string, string> { ["id"] = "2" }]),
        };

        using var writer = new System.IO.StringWriter();
        using var csv = new CsvWriter(writer, System.Globalization.CultureInfo.InvariantCulture);
        _exporter.WriteConsolidatedCsv(csv, results);
        csv.Flush();

        var lines = writer.ToString().Split('\n', StringSplitOptions.RemoveEmptyEntries);
        var lastDataLine = lines[2].TrimEnd('\r');
        Assert.IsTrue(lastDataLine.EndsWith(","), $"Expected trailing comma for empty name column but got: {lastDataLine}");
    }

    [TestMethod(DisplayName = "AppendToServerCsv creates file with header on first write")]
    public void AppendToServerCsv_FirstWrite_CreatesHeaderAndData()
    {
        var row = CreateSuccessRow("prod", "db1", ["id", "name"],
        [
            new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" }
        ]);
        var outputPath = IO.Combine(_testDirectory, "server_prod.csv");

        _exporter.AppendToServerCsv(outputPath, row);

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual(2, lines.Length);
        Assert.AreEqual("Server,Database,id,name", lines[0]);
        Assert.IsTrue(lines[1].StartsWith("prod,db1,"));
    }

    [TestMethod(DisplayName = "AppendToServerCsv appends without rewriting header")]
    public void AppendToServerCsv_SubsequentWrite_AppendsWithoutHeader()
    {
        var row1 = CreateSuccessRow("prod", "db1", ["id", "name"],
        [
            new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" }
        ]);
        var row2 = CreateSuccessRow("prod", "db2", ["id", "name"],
        [
            new Dictionary<string, string> { ["id"] = "10", ["name"] = "Bob" }
        ]);
        var outputPath = IO.Combine(_testDirectory, "server_prod.csv");

        _exporter.AppendToServerCsv(outputPath, row1);
        _exporter.AppendToServerCsv(outputPath, row2);

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual(3, lines.Length);
        Assert.AreEqual("Server,Database,id,name", lines[0]);
        Assert.IsTrue(lines[1].StartsWith("prod,db1,"));
        Assert.IsTrue(lines[2].StartsWith("prod,db2,"));
    }

    [TestMethod(DisplayName = "AppendToServerCsv with different columns results in inconsistent header (acceptable)")]
    public void AppendToServerCsv_DifferentColumns_HeaderInconsistent()
    {
        var row1 = CreateSuccessRow("prod", "db1", ["id", "name"],
        [
            new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" }
        ]);
        var row2 = CreateSuccessRow("prod", "db2", ["id", "email"],
        [
            new Dictionary<string, string> { ["id"] = "2", ["email"] = "bob@test.com" }
        ]);
        var outputPath = IO.Combine(_testDirectory, "server_prod.csv");

        _exporter.AppendToServerCsv(outputPath, row1);
        _exporter.AppendToServerCsv(outputPath, row2);

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual(3, lines.Length);
        Assert.AreEqual("Server,Database,id,name", lines[0]);
        Assert.IsTrue(lines[1].Contains("Alice"));
        Assert.IsTrue(lines[2].Contains("bob@test.com"));
    }

    [TestMethod(DisplayName = "WriteErrorEntry creates error file with header and appends")]
    public void WriteErrorEntry_CreatesAndAppendsErrors()
    {
        var outputPath = IO.Combine(_testDirectory, "errors.csv");

        _exporter.WriteErrorEntry(outputPath, "prod", "db1", DateTime.UtcNow, "connection refused");
        _exporter.WriteErrorEntry(outputPath, "prod", "db2", DateTime.UtcNow, "timeout");

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual(3, lines.Length);
        Assert.AreEqual("Server,Database,ExecutedAt,Error", lines[0]);
        Assert.IsTrue(lines[1].Contains("prod"));
        Assert.IsTrue(lines[1].Contains("connection refused"));
        Assert.IsTrue(lines[2].Contains("timeout"));
    }

    [TestMethod(DisplayName = "WriteLogEntry creates execution log with header and appends")]
    public void WriteLogEntry_CreatesAndAppendsLog()
    {
        var outputPath = IO.Combine(_testDirectory, "log.csv");

        _exporter.WriteLogEntry(outputPath, new ExecutionLogEntry("prod", "db1", DateTime.UtcNow, "Success", 100, 250.5, string.Empty));
        _exporter.WriteLogEntry(outputPath, new ExecutionLogEntry("prod", "db2", DateTime.UtcNow, "Error", 0, 50.0, "timeout"));

        var lines = IOFile.ReadAllLines(outputPath);
        Assert.AreEqual(3, lines.Length);
        Assert.AreEqual("Server,Database,ExecutedAt,Status,RowCount,DurationMs,Error", lines[0]);
        Assert.IsTrue(lines[1].Contains("Success"));
        Assert.IsTrue(lines[1].Contains("250.5"));
        Assert.IsTrue(lines[2].Contains("Error"));
        Assert.IsTrue(lines[2].Contains("timeout"));
    }

    [TestMethod(DisplayName = "SanitizeFilename replaces invalid characters with underscores")]
    public void SanitizeFilename_ReplacesInvalidChars()
    {
        var result = _exporter.SanitizeFilename("my/server:db*test");
        Assert.IsFalse(result.Contains('/'));
        Assert.IsFalse(result.Contains(':'));
        Assert.IsFalse(result.Contains('*'));
        Assert.IsTrue(result.Contains("_"));
    }

    [TestMethod(DisplayName = "SanitizeFilename keeps valid name unchanged")]
    public void SanitizeFilename_ValidName_Unchanged()
    {
        var result = _exporter.SanitizeFilename("prod-pg-01");
        Assert.AreEqual("prod-pg-01", result);
    }

    [TestMethod(DisplayName = "MergeServerCsvsToConsolidated generates consolidated CSV with unified header")]
    public void MergeServerCsvs_GeneratesConsolidatedWithUnifiedHeader()
    {
        var timestamp = "2026-04-13_143022";
        var row1 = CreateSuccessRow("server1", "db1", ["id", "name"],
        [
            new Dictionary<string, string> { ["id"] = "1", ["name"] = "Alice" }
        ]);
        var row2 = CreateSuccessRow("server2", "db1", ["id", "email"],
        [
            new Dictionary<string, string> { ["id"] = "2", ["email"] = "bob@test.com" }
        ]);

        var server1File = IO.Combine(_testDirectory, $"server1_{timestamp}.csv");
        var server2File = IO.Combine(_testDirectory, $"server2_{timestamp}.csv");

        _exporter.AppendToServerCsv(server1File, row1);
        _exporter.AppendToServerCsv(server2File, row2);

        _exporter.MergeServerCsvsToConsolidated(_testDirectory, timestamp, ["server1", "server2"]);

        var consolidatedPath = IO.Combine(_testDirectory, $"consolidated_{timestamp}.csv");
        Assert.IsTrue(IOFile.Exists(consolidatedPath));

        var lines = IOFile.ReadAllLines(consolidatedPath);
        Assert.AreEqual(3, lines.Length);
        Assert.AreEqual("Server,Database,id,name,email", lines[0]);
    }

    [TestMethod(DisplayName = "Immediate flush — data persisted after each write")]
    public void AppendToServerCsv_FlushImmediate_DataOnDisk()
    {
        var row = CreateSuccessRow("prod", "db1", ["id"],
        [
            new Dictionary<string, string> { ["id"] = "1" }
        ]);
        var outputPath = IO.Combine(_testDirectory, "flush_test.csv");

        _exporter.AppendToServerCsv(outputPath, row);

        Assert.IsTrue(IOFile.Exists(outputPath));
        var content = IOFile.ReadAllText(outputPath);
        Assert.IsTrue(content.Contains("prod"), "Data should be flushed to disk immediately after write");
    }
}
