using System.Text;

using FurLab.Core.Constants;
using FurLab.Core.Interfaces;
using FurLab.Core.Models;

using Microsoft.Extensions.FileSystemGlobbing;
using Microsoft.Extensions.FileSystemGlobbing.Abstractions;

using UtfUnknown;

namespace FurLab.Core.Services;

/// <summary>
/// Provides encoding conversion services for files.
/// </summary>
public class EncodingConversionService : IEncodingConversionService
{
    /// <summary>
    /// Converts files matching the specified pattern to the target encoding.
    /// </summary>
    /// <param name="options">The conversion options.</param>
    /// <param name="progress">Optional progress reporter.</param>
    /// <param name="cancellationToken">Optional cancellation token.</param>
    /// <returns>A task representing the operation with the conversion result.</returns>
    public async Task<EncodingConversionResult> ConvertFilesAsync(
        EncodingConversionOptions options,
        IProgress<EncodingConversionProgress>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(options);

        var convertedFiles = new List<ProcessedFileInfo>();
        var errors = new List<ConversionErrorInfo>();

        // Find matching files
        var files = FindMatchingFiles(options, out var baseDirectory);
        var totalFiles = files.Count;
        var processedCount = 0;

        if (totalFiles == 0)
        {
            return new EncodingConversionResult
            {
                TotalFiles = 0,
                ConvertedCount = 0,
                SkippedCount = 0,
                ErrorCount = 0,
            };
        }

        // Get target encoding
        var targetEncoding = GetEncoding(options.TargetEncoding) ?? throw new ArgumentException($"Unsupported or invalid target encoding: '{options.TargetEncoding}'");

        foreach (var file in files)
        {
            cancellationToken.ThrowIfCancellationRequested();

            processedCount++;
            progress?.Report(new EncodingConversionProgress
            {
                TotalFiles = totalFiles,
                ProcessedFiles = processedCount,
                CurrentFile = file,
                Message = $"Processing {Path.GetFileName(file)}...",
            });

            try
            {
                var result = await ConvertSingleFileAsync(file, baseDirectory, options, targetEncoding, cancellationToken);

                if (result is not null)
                {
                    convertedFiles.Add(result);
                }
            }
            catch (Exception ex)
            {
                errors.Add(new ConversionErrorInfo
                {
                    FilePath = file,
                    ErrorMessage = ex.Message,
                    ErrorType = ConversionErrorType.Unknown,
                });
            }
        }

        return new EncodingConversionResult
        {
            TotalFiles = totalFiles,
            ConvertedCount = convertedFiles.Count(f => f.WasConverted),
            SkippedCount = convertedFiles.Count(f => !f.WasConverted),
            ErrorCount = errors.Count,
            ProcessedFiles = convertedFiles,
            Errors = errors,
        };
    }

    private static List<string> FindMatchingFiles(EncodingConversionOptions options, out string baseDirectory)
    {
        var fullPattern = Path.GetFullPath(options.Pattern);

        // If pattern is a full file path (not a glob), return it directly if it exists
        if (File.Exists(fullPattern))
        {
            baseDirectory = Path.GetDirectoryName(fullPattern) ?? Environment.CurrentDirectory;
            if (options.TextOnly && !TextFileExtensions.IsTextFile(fullPattern))
            {
                return [];
            }

            return [fullPattern];
        }

        baseDirectory = GetBaseDirectory(fullPattern);
        var relativePattern = Path.GetRelativePath(baseDirectory, fullPattern);

        var matcher = new Matcher();
        matcher.AddInclude(relativePattern);

        foreach (var excludePattern in options.ExcludePatterns)
        {
            matcher.AddExclude(excludePattern);
        }

        // Use glob matching
        var directoryInfo = new DirectoryInfo(baseDirectory);
        if (!directoryInfo.Exists)
        {
            return [];
        }

        var localBaseDir = baseDirectory;
        var result = matcher.Execute(new DirectoryInfoWrapper(directoryInfo));
        var files = result.Files
            .Select(f => Path.Combine(localBaseDir, f.Path))
            .Where(f => File.Exists(f));

        // Filter by text-only if requested
        if (options.TextOnly)
        {
            files = files.Where(TextFileExtensions.IsTextFile);
        }

        return [.. files];
    }

    private static string GetBaseDirectory(string pattern)
    {
        var firstGlobIndex = pattern.IndexOfAny(['*', '?']);
        if (firstGlobIndex == -1)
        {
            var dir = Path.GetDirectoryName(pattern);
            return string.IsNullOrEmpty(dir) ? Environment.CurrentDirectory : Path.GetFullPath(dir);
        }

        var partBeforeGlob = pattern[..firstGlobIndex];
        var lastSeparatorBeforeGlob = partBeforeGlob.LastIndexOfAny([Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar]);

        if (lastSeparatorBeforeGlob == -1)
        {
            return Environment.CurrentDirectory;
        }

        var baseDir = partBeforeGlob[..lastSeparatorBeforeGlob];

        if (string.IsNullOrEmpty(baseDir))
        {
            return Path.GetPathRoot(pattern) ?? (Path.IsPathRooted(pattern) ? "/" : Environment.CurrentDirectory);
        }

        if (baseDir.EndsWith(':'))
        {
            return baseDir + Path.DirectorySeparatorChar;
        }

        return Path.GetFullPath(baseDir);
    }

    private static async Task<ProcessedFileInfo?> ConvertSingleFileAsync(
        string filePath,
        string baseDirectory,
        EncodingConversionOptions options,
        Encoding targetEncoding,
        CancellationToken cancellationToken)
    {
        // Read file bytes
        byte[] fileBytes;
        DateTime originalCreationTime;
        DateTime originalLastWriteTime;
        DateTime originalLastAccessTime;

        try
        {
            var fileInfo = new FileInfo(filePath);
            originalCreationTime = fileInfo.CreationTime;
            originalLastWriteTime = fileInfo.LastWriteTime;
            originalLastAccessTime = fileInfo.LastAccessTime;
            fileBytes = await File.ReadAllBytesAsync(filePath, cancellationToken);
        }
        catch (FileNotFoundException)
        {
            throw new InvalidOperationException($"File not found: {filePath}");
        }
        catch (UnauthorizedAccessException)
        {
            throw new InvalidOperationException($"Permission denied when reading file: {filePath}");
        }

        // Detect source encoding
        var detectionResult = DetectEncoding(fileBytes, options.SourceEncoding);

        if (detectionResult.Encoding is null)
        {
            if (!options.Force)
            {
                throw new InvalidOperationException($"Could not detect encoding for file: {filePath}. Use --force to convert anyway.");
            }

            // Default to Latin1 if detection fails and force is enabled
            detectionResult = new EncodingDetectionResult
            {
                Encoding = Encoding.GetEncoding("ISO-8859-1"),
                Confidence = 0.0,
                EncodingName = "ISO-8859-1",
            };
        }

        if (detectionResult.Confidence <= options.ConfidenceThreshold && !options.Force && string.IsNullOrEmpty(options.SourceEncoding))
        {
            throw new InvalidOperationException($"Low confidence ({detectionResult.Confidence:P}) when detecting encoding for file: {filePath}. Use --force to convert anyway or specify --from.");
        }

        // Check if already in target encoding
        var targetEncodingName = GetEncodingName(targetEncoding);

        // Handle ASCII to UTF-8 compatibility (no conversion needed if only ASCII chars)
        var isAlreadyInTarget = detectionResult.EncodingName.Equals(targetEncodingName, StringComparison.OrdinalIgnoreCase) ||
                                (targetEncodingName == "UTF-8" && detectionResult.EncodingName == "US-ASCII");

        if (isAlreadyInTarget)
        {
            return new ProcessedFileInfo
            {
                OriginalPath = filePath,
                OutputPath = filePath,
                SourceEncoding = detectionResult.EncodingName,
                TargetEncoding = targetEncodingName,
                DetectionConfidence = detectionResult.Confidence,
                WasConverted = false,
            };
        }

        // Convert content
        var content = detectionResult.Encoding.GetString(fileBytes);
        var targetBytes = targetEncoding.GetBytes(content);

        // Determine output path
        var outputPath = filePath;
        if (!string.IsNullOrEmpty(options.OutputDirectory))
        {
            var relativePath = Path.GetRelativePath(baseDirectory, filePath);
            outputPath = Path.Combine(options.OutputDirectory, relativePath);

            var outputDir = Path.GetDirectoryName(outputPath);
            if (!string.IsNullOrEmpty(outputDir))
            {
                Directory.CreateDirectory(outputDir);
            }
        }

        // Create backup if requested
        string? backupPath = null;
        if (options.CreateBackup)
        {
            backupPath = outputPath + ".bak";
            File.Copy(filePath, backupPath, overwrite: true);
        }

        // Write with safe strategy (temp file first)
        var tempPath = outputPath + ".tmp";
        try
        {
            await File.WriteAllBytesAsync(tempPath, targetBytes, cancellationToken);

            // Replace original with temp atomically
            File.Move(tempPath, outputPath, overwrite: true);

            // Preserve timestamps
            File.SetCreationTime(outputPath, originalCreationTime);
            File.SetLastWriteTime(outputPath, originalLastWriteTime);
            File.SetLastAccessTime(outputPath, originalLastAccessTime);
        }
        catch (Exception)
        {
            // Clean up temp file if something went wrong
            if (File.Exists(tempPath))
            {
                File.Delete(tempPath);
            }

            throw;
        }

        return new ProcessedFileInfo
        {
            OriginalPath = filePath,
            OutputPath = outputPath,
            SourceEncoding = detectionResult.EncodingName,
            TargetEncoding = targetEncodingName,
            DetectionConfidence = detectionResult.Confidence,
            BackupPath = backupPath,
            WasConverted = true,
        };
    }

    private static EncodingDetectionResult DetectEncoding(byte[] bytes, string? explicitEncoding)
    {
        // If explicit encoding is specified, use it
        if (!string.IsNullOrEmpty(explicitEncoding))
        {
            var explicitEnc = GetEncoding(explicitEncoding);
            if (explicitEnc is null)
            {
                throw new ArgumentException($"Unsupported or invalid source encoding: '{explicitEncoding}'");
            }

            return new EncodingDetectionResult
            {
                Encoding = explicitEnc,
                Confidence = 1.0,
                EncodingName = explicitEncoding,
            };
        }

        // Try BOM detection first (100% reliable)
        var bomEncoding = DetectBom(bytes);
        if (bomEncoding is not null)
        {
            return new EncodingDetectionResult
            {
                Encoding = bomEncoding,
                Confidence = 1.0,
                EncodingName = GetEncodingName(bomEncoding),
            };
        }

        // Use UTF.Unknown for detection
        var detectionResult = CharsetDetector.DetectFromBytes(bytes);

        if (detectionResult?.Detected is null || detectionResult.Detected.Encoding is null)
        {
            return new EncodingDetectionResult
            {
                Encoding = null,
                Confidence = 0.0,
                EncodingName = "Unknown",
            };
        }

        return new EncodingDetectionResult
        {
            Encoding = detectionResult.Detected.Encoding,
            Confidence = detectionResult.Detected.Confidence,
            EncodingName = GetEncodingName(detectionResult.Detected.Encoding),
        };
    }

    private static Encoding? DetectBom(byte[] bytes)
    {
        if (bytes.Length < 2)
        {
            return null;
        }

        // UTF-8 BOM: EF BB BF
        if (bytes.Length >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF)
        {
            return new UTF8Encoding(encoderShouldEmitUTF8Identifier: true);
        }

        // UTF-16 LE BOM: FF FE
        if (bytes[0] == 0xFF && bytes[1] == 0xFE)
        {
            return bytes.Length >= 4 && bytes[2] == 0x00 && bytes[3] == 0x00
                ? Encoding.UTF32 // UTF-32 LE
                : Encoding.Unicode; // UTF-16 LE
        }

        // UTF-16 BE BOM: FE FF
        if (bytes[0] == 0xFE && bytes[1] == 0xFF)
        {
            return Encoding.BigEndianUnicode;
        }

        // UTF-32 BE BOM: 00 00 FE FF
        if (bytes.Length >= 4 && bytes[0] == 0x00 && bytes[1] == 0x00 && bytes[2] == 0xFE && bytes[3] == 0xFF)
        {
            return new UTF32Encoding(bigEndian: true, byteOrderMark: true);
        }

        return null;
    }

    private static Encoding? GetEncoding(string encodingName)
    {
        try
        {
            return encodingName.ToUpperInvariant() switch
            {
                "UTF-8" or "UTF8" => new UTF8Encoding(encoderShouldEmitUTF8Identifier: false),
                "UTF-8-BOM" or "UTF8-BOM" => new UTF8Encoding(encoderShouldEmitUTF8Identifier: true),
                "UTF-16" or "UTF16" or "UNICODE" => Encoding.Unicode,
                "UTF-16-LE" or "UTF16-LE" => Encoding.Unicode,
                "UTF-16-BE" or "UTF16-BE" => Encoding.BigEndianUnicode,
                "UTF-32" or "UTF32" => Encoding.UTF32,
                "UTF-32-LE" or "UTF32-LE" => Encoding.UTF32,
                "UTF-32-BE" or "UTF32-BE" => new UTF32Encoding(bigEndian: true, byteOrderMark: true),
                "ASCII" => Encoding.ASCII,
                "LATIN1" or "ISO-8859-1" => Encoding.GetEncoding("ISO-8859-1"),
                "WINDOWS-1252" or "CP1252" => Encoding.GetEncoding("Windows-1252"),
                _ => Encoding.GetEncoding(encodingName), // Try to get by name
            };
        }
        catch (ArgumentException)
        {
            return null;
        }
    }

    private static string GetEncodingName(Encoding encoding)
    {
        if (encoding is UTF8Encoding utf8)
        {
            return utf8.GetPreamble().Length > 0 ? "UTF-8-BOM" : "UTF-8";
        }

        if (encoding == Encoding.Unicode)
        {
            return "UTF-16-LE";
        }

        if (encoding == Encoding.BigEndianUnicode)
        {
            return "UTF-16-BE";
        }

        if (encoding == Encoding.UTF32)
        {
            return "UTF-32-LE";
        }

        if (encoding.WebName.Equals("iso-8859-1", StringComparison.OrdinalIgnoreCase))
        {
            return "ISO-8859-1";
        }

        return encoding.WebName.ToUpperInvariant();
    }

    private class EncodingDetectionResult
    {
        public Encoding? Encoding { get; init; }
        public double Confidence { get; init; }
        public string EncodingName { get; init; } = "Unknown";
    }
}
