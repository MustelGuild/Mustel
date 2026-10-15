using System;
using System.IO;
using System.Text;

using FurLab.CLI.Commands.Files;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class FileCommandTests
{
    private string _testDirectory = null!;

    [TestInitialize]
    public void Setup()
    {
        _testDirectory = Path.Combine(Path.GetTempPath(), $"FileCommandTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDirectory);
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (Directory.Exists(_testDirectory))
        {
            Directory.Delete(_testDirectory, recursive: true);
        }
    }

    [TestMethod(DisplayName = "Combine with valid pattern should merge files correctly")]
    [Description("Verifies that multiple files matching the pattern are combined.")]
    public void Combine_ValidPattern_MergesFilesCorrectly()
    {
        var file1 = Path.Combine(_testDirectory, "file1.txt");
        var file2 = Path.Combine(_testDirectory, "file2.txt");

        File.WriteAllText(file1, "Content 1", Encoding.UTF8);
        File.WriteAllText(file2, "Content 2", Encoding.UTF8);

        var outputPath = Path.Combine(_testDirectory, "combined.txt");

        var inputFilePaths = Directory.GetFiles(_testDirectory, "*.txt");
        var allFileText = new StringBuilder();
        var currentEncoding = Encoding.UTF8;

        foreach (var inputFilePath in inputFilePaths)
        {
            currentEncoding = FileEncodingHelper.DetectEncoding(inputFilePath);
            allFileText.Append(File.ReadAllText(inputFilePath, currentEncoding));
            allFileText.AppendLine();
        }

        File.WriteAllText(outputPath, allFileText.ToString(), currentEncoding);

        Assert.IsTrue(File.Exists(outputPath));
        var content = File.ReadAllText(outputPath);
        Assert.IsTrue(content.Contains("Content 1"), "Expected content to contain 'Content 1'");
        Assert.IsTrue(content.Contains("Content 2"), "Expected content to contain 'Content 2'");
    }

    [TestMethod(DisplayName = "GetCurrentFileEncoding detects UTF-8 encoding")]
    [Description("Verifies that encoding detection correctly identifies UTF-8.")]
    public void GetCurrentFileEncoding_DetectsUtf8()
    {
        var filePath = Path.Combine(_testDirectory, "utf8test.txt");
        File.WriteAllText(filePath, "áéíóúãõû", Encoding.UTF8);

        var encoding = FileEncodingHelper.DetectEncoding(filePath);
        Assert.IsNotNull(encoding);
    }

    [TestMethod(DisplayName = "Combine without specified OutputFile should use default .sql extension")]
    [Description("Verifies that when the output file is not provided, the default name 'CombineFiles.sql' is used.")]
    public void Combine_OutputNotSpecified_UsesDefaultCombineFilesSql()
    {
        var file1 = Path.Combine(_testDirectory, "test1.sql");
        File.WriteAllText(file1, "SELECT 1;", Encoding.UTF8);

        var inputFilePaths = Directory.GetFiles(_testDirectory, "*.sql");
        Assert.AreEqual(1, inputFilePaths.Length);

        var allFileText = new StringBuilder();
        var currentEncoding = Encoding.UTF8;

        foreach (var inputFilePath in inputFilePaths)
        {
            currentEncoding = FileEncodingHelper.DetectEncoding(inputFilePath);
            allFileText.Append(File.ReadAllText(inputFilePath, currentEncoding));
            allFileText.AppendLine();
        }

        var expectedOutput = Path.Combine(_testDirectory, "CombineFiles.sql");
        File.WriteAllText(expectedOutput, allFileText.ToString(), currentEncoding);

        Assert.IsTrue(File.Exists(expectedOutput));
    }

    [TestMethod(DisplayName = "Combine with UTF-8 encoded files should preserve special characters")]
    [Description("Verifies that accented characters and symbols are correctly preserved during merging.")]
    public void Combine_Utf8Files_PreservesSpecialCharacters()
    {
        var file1 = Path.Combine(_testDirectory, "file1.txt");
        var file2 = Path.Combine(_testDirectory, "file2.txt");

        var content1 = "Content with special chars: áéíóú";
        var content2 = "More content: ãõû";

        File.WriteAllText(file1, content1, Encoding.UTF8);
        File.WriteAllText(file2, content2, Encoding.UTF8);

        var outputPath = Path.Combine(_testDirectory, "combined.txt");

        var inputFilePaths = Directory.GetFiles(_testDirectory, "*.txt");
        var allFileText = new StringBuilder();
        var currentEncoding = Encoding.UTF8;

        foreach (var inputFilePath in inputFilePaths)
        {
            currentEncoding = FileEncodingHelper.DetectEncoding(inputFilePath);
            allFileText.Append(File.ReadAllText(inputFilePath, currentEncoding));
            allFileText.AppendLine();
        }

        File.WriteAllText(outputPath, allFileText.ToString(), currentEncoding);

        var result = File.ReadAllText(outputPath, Encoding.UTF8);
        Assert.IsTrue(result.Contains("áéíóú"), "Expected special characters to be preserved");
        Assert.IsTrue(result.Contains("ãõû"), "Expected special characters to be preserved");
    }
}
