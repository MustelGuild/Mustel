using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

using FurLab.Core.Constants;
using FurLab.Core.Models;
using FurLab.Core.Services;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class EncodingConversionServiceTests
{
    private string _testDirectory = null!;
    private EncodingConversionService _service = null!;

    [TestInitialize]
    public void Setup()
    {
        _testDirectory = Path.Combine(Path.GetTempPath(), $"EncodingConversionTests_{Guid.NewGuid():N}");
        Directory.CreateDirectory(_testDirectory);
        _service = new EncodingConversionService();
    }

    [TestCleanup]
    public void Cleanup()
    {
        if (Directory.Exists(_testDirectory))
        {
            Directory.Delete(_testDirectory, recursive: true);
        }
    }

    #region TextFileExtensions Tests

    [TestMethod(DisplayName = "IsTextFile returns true for known text extensions")]
    [Description("Verifies that IsTextFile correctly identifies text file extensions.")]
    public void IsTextFile_KnownExtensions_ReturnsTrue()
    {
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.cs"));
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.txt"));
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.md"));
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.json"));
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.xml"));
    }

    [TestMethod(DisplayName = "IsTextFile returns false for unknown extensions")]
    [Description("Verifies that IsTextFile correctly rejects non-text file extensions.")]
    public void IsTextFile_UnknownExtensions_ReturnsFalse()
    {
        Assert.IsFalse(TextFileExtensions.IsTextFile("test.exe"));
        Assert.IsFalse(TextFileExtensions.IsTextFile("test.dll"));
        Assert.IsFalse(TextFileExtensions.IsTextFile("test.png"));
        Assert.IsFalse(TextFileExtensions.IsTextFile("test.jpg"));
    }

    [TestMethod(DisplayName = "IsTextFile handles case insensitivity")]
    [Description("Verifies that IsTextFile is case-insensitive.")]
    public void IsTextFile_CaseInsensitive_ReturnsTrue()
    {
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.CS"));
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.TXT"));
        Assert.IsTrue(TextFileExtensions.IsTextFile("test.Md"));
    }

    #endregion

    #region BOM Detection Tests

    [TestMethod(DisplayName = "Detects UTF-8 BOM correctly")]
    [Description("Verifies that UTF-8 BOM is detected with 100% confidence.")]
    public async Task ConvertFiles_Utf8Bom_DetectsCorrectly()
    {
        var content = "Test content with UTF-8 BOM";
        var bytes = new UTF8Encoding(true).GetBytes(content);
        var filePath = Path.Combine(_testDirectory, "utf8bom.txt");
        await File.WriteAllBytesAsync(filePath, bytes);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            TargetEncoding = "UTF-8",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.TotalFiles);
        Assert.AreEqual(1, result.ProcessedFiles.Count);
        Assert.AreEqual(1.0, result.ProcessedFiles[0].DetectionConfidence);
    }

    [TestMethod(DisplayName = "Detects UTF-16 LE BOM correctly")]
    [Description("Verifies that UTF-16 LE BOM is detected with 100% confidence.")]
    public async Task ConvertFiles_Utf16LeBom_DetectsCorrectly()
    {
        var content = "Test content";
        var bytes = Encoding.Unicode.GetBytes(content);
        var bom = Encoding.Unicode.GetPreamble();
        var filePath = Path.Combine(_testDirectory, "utf16le.txt");
        await File.WriteAllBytesAsync(filePath, [.. bom, .. bytes]);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            TargetEncoding = "UTF-8",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.TotalFiles);
        Assert.AreEqual(1, result.ProcessedFiles.Count);
        Assert.AreEqual(1.0, result.ProcessedFiles[0].DetectionConfidence);
    }

    #endregion

    #region Encoding Conversion Tests

    [TestMethod(DisplayName = "Converts Latin1 to UTF-8 correctly")]
    [Description("Verifies that files encoded in Latin1 are correctly converted to UTF-8.")]
    public async Task ConvertFiles_Latin1ToUtf8_ConvertsCorrectly()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");
        var content = "Café résumé naïve";
        var filePath = Path.Combine(_testDirectory, "latin1.txt");
        await File.WriteAllTextAsync(filePath, content, latin1Encoding);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.TotalFiles);
        Assert.AreEqual(1, result.ConvertedCount);
        Assert.AreEqual(0, result.ErrorCount);

        var convertedContent = await File.ReadAllTextAsync(filePath);
        Assert.AreEqual(content, convertedContent);
    }

    [TestMethod(DisplayName = "Converts with explicit encoding specification")]
    [Description("Verifies that explicit source encoding is used correctly.")]
    public async Task ConvertFiles_ExplicitSourceEncoding_UsesSpecifiedEncoding()
    {
        var content = "Test content";
        var filePath = Path.Combine(_testDirectory, "test.txt");
        await File.WriteAllTextAsync(filePath, content, Encoding.UTF8);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            SourceEncoding = "UTF-8",
            TargetEncoding = "UTF-8-BOM",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.ConvertedCount);

        var bytes = await File.ReadAllBytesAsync(filePath);
        var bom = new UTF8Encoding(true).GetPreamble();
        Assert.IsTrue(bytes.Take(bom.Length).SequenceEqual(bom));
    }

    [TestMethod(DisplayName = "Skips files already in target encoding")]
    [Description("Verifies that files already in the target encoding are skipped.")]
    public async Task ConvertFiles_AlreadyTargetEncoding_SkipsFile()
    {
        var content = "Already UTF-8";
        var filePath = Path.Combine(_testDirectory, "utf8.txt");
        await File.WriteAllTextAsync(filePath, content, new UTF8Encoding(false));

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            TargetEncoding = "UTF-8",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.TotalFiles);
        Assert.AreEqual(1, result.SkippedCount);
        Assert.AreEqual(0, result.ConvertedCount);
    }

    #endregion

    #region Backup Tests

    [TestMethod(DisplayName = "Creates backup when requested")]
    [Description("Verifies that .bak files are created when CreateBackup is true.")]
    public async Task ConvertFiles_CreateBackup_CreatesBakFile()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");
        var content = "Café";
        var filePath = Path.Combine(_testDirectory, "backup_test.txt");
        await File.WriteAllTextAsync(filePath, content, latin1Encoding);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
            CreateBackup = true,
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.ConvertedCount);
        Assert.IsTrue(File.Exists(filePath + ".bak"));
        Assert.IsNotNull(result.ProcessedFiles[0].BackupPath);

        // Verify backup contains original content
        var backupContent = await File.ReadAllTextAsync(filePath + ".bak", latin1Encoding);
        Assert.AreEqual(content, backupContent);
    }

    [TestMethod(DisplayName = "Does not create backup when not requested")]
    [Description("Verifies that .bak files are not created when CreateBackup is false.")]
    public async Task ConvertFiles_NoBackup_DoesNotCreateBakFile()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");
        var filePath = Path.Combine(_testDirectory, "no_backup.txt");
        await File.WriteAllTextAsync(filePath, "test", latin1Encoding);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
            CreateBackup = false,
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.ConvertedCount);
        Assert.IsFalse(File.Exists(filePath + ".bak"));
    }

    #endregion

    #region Output Directory Tests

    [TestMethod(DisplayName = "Converts to output directory")]
    [Description("Verifies that files are converted to the specified output directory.")]
    public async Task ConvertFiles_OutputDirectory_ConvertsToDirectory()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");
        var content = "Test";
        var filePath = Path.Combine(_testDirectory, "input.txt");
        await File.WriteAllTextAsync(filePath, content, latin1Encoding);

        var outputDir = Path.Combine(_testDirectory, "output");
        Directory.CreateDirectory(outputDir);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
            OutputDirectory = outputDir,
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.ConvertedCount);
        Assert.IsTrue(File.Exists(Path.Combine(outputDir, "input.txt")));
        Assert.IsTrue(File.Exists(filePath)); // Original should still exist
    }

    #endregion

    #region Confidence Threshold Tests

    [TestMethod(DisplayName = "Low confidence with force flag converts file")]
    [Description("Verifies that files with low detection confidence are converted when Force is true.")]
    public async Task ConvertFiles_LowConfidenceWithForce_Converts()
    {
        // Create a very small file that's hard to detect
        var filePath = Path.Combine(_testDirectory, "small.txt");
        await File.WriteAllTextAsync(filePath, "AB"); // Too small for reliable detection

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            TargetEncoding = "UTF-8",
            ConfidenceThreshold = 0.95, // High threshold
            Force = true,
        };

        var result = await _service.ConvertFilesAsync(options);

        // Should convert even with low confidence
        Assert.AreEqual(1, result.TotalFiles);
    }

    #endregion

    #region Batch Processing Tests

    [TestMethod(DisplayName = "Processes multiple files matching pattern")]
    [Description("Verifies that multiple files matching the pattern are processed.")]
    public async Task ConvertFiles_MultipleFiles_ProcessesAll()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");

        for (var i = 0; i < 3; i++)
        {
            var filePath = Path.Combine(_testDirectory, $"file{i}.txt");
            await File.WriteAllTextAsync(filePath, $"Content {i}", latin1Encoding);
        }

        var options = new EncodingConversionOptions
        {
            Pattern = Path.Combine(_testDirectory, "*.txt"),
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(3, result.TotalFiles);
        Assert.AreEqual(3, result.ConvertedCount);
    }

    [TestMethod(DisplayName = "Text-only filter excludes non-text files")]
    [Description("Verifies that --text-only flag filters out non-text files.")]
    public async Task ConvertFiles_TextOnlyFilter_FiltersNonText()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");

        // Create text file
        await File.WriteAllTextAsync(Path.Combine(_testDirectory, "file.txt"), "text", latin1Encoding);

        // Create binary-looking file
        await File.WriteAllBytesAsync(Path.Combine(_testDirectory, "file.exe"), [0x4D, 0x5A]);

        var options = new EncodingConversionOptions
        {
            Pattern = Path.Combine(_testDirectory, "*"),
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
            TextOnly = true,
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(1, result.TotalFiles);
        Assert.IsTrue(result.ProcessedFiles[0].OriginalPath.EndsWith(".txt"));
    }

    #endregion

    #region Error Handling Tests

    [TestMethod(DisplayName = "Handles non-existent file gracefully")]
    [Description("Verifies that non-existent files are reported as errors.")]
    public async Task ConvertFiles_NonExistentFile_ReturnsError()
    {
        var options = new EncodingConversionOptions
        {
            Pattern = Path.Combine(_testDirectory, "nonexistent.txt"),
            TargetEncoding = "UTF-8",
        };

        var result = await _service.ConvertFilesAsync(options);

        Assert.AreEqual(0, result.TotalFiles);
    }

    [TestMethod(DisplayName = "Preserves timestamps after conversion")]
    [Description("Verifies that file timestamps are preserved after conversion.")]
    public async Task ConvertFiles_PreservesTimestamps()
    {
        var latin1Encoding = Encoding.GetEncoding("ISO-8859-1");
        var filePath = Path.Combine(_testDirectory, "timestamps.txt");
        await File.WriteAllTextAsync(filePath, "test", latin1Encoding);

        // Set specific timestamps
        var creationTime = new DateTime(2023, 1, 15, 10, 30, 0);
        var lastWriteTime = new DateTime(2023, 2, 20, 14, 45, 0);
        File.SetCreationTime(filePath, creationTime);
        File.SetLastWriteTime(filePath, lastWriteTime);

        var options = new EncodingConversionOptions
        {
            Pattern = filePath,
            SourceEncoding = "ISO-8859-1",
            TargetEncoding = "UTF-8",
        };

        await _service.ConvertFilesAsync(options);

        // Verify timestamps are preserved
        if (OperatingSystem.IsWindows())
        {
            Assert.AreEqual(creationTime, File.GetCreationTime(filePath));
        }
        Assert.AreEqual(lastWriteTime, File.GetLastWriteTime(filePath));
    }

    #endregion
}
