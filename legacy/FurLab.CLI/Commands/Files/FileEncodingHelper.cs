using System.Text;

namespace FurLab.CLI.Commands.Files;

/// <summary>
/// Helper methods for file operations, specifically for encoding detection.
/// </summary>
public static class FileEncodingHelper
{
    /// <summary>
    /// Detects and returns the encoding of a file.
    /// </summary>
    /// <param name="filePath">The path to the file.</param>
    /// <returns>The detected <see cref="Encoding"/> of the file.</returns>
    public static Encoding DetectEncoding(string filePath)
    {
        using var sr = new StreamReader(filePath, true);
        while (sr.Peek() >= 0)
        {
            sr.Read();
        }

        return sr.CurrentEncoding;
    }
}
