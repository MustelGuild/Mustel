namespace FurLab.Core.Constants;

/// <summary>
/// Defines known text file extensions for encoding conversion.
/// </summary>
public static class TextFileExtensions
{
    /// <summary>
    /// Gets the set of known text file extensions.
    /// </summary>
    public static readonly HashSet<string> KnownExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        // C# / .NET
        ".cs",
        ".cshtml",
        ".razor",
        ".vb",
        ".fs",
        ".fsx",
        ".csproj",
        ".sln",
        ".props",
        ".targets",
        ".config",
        ".settings",
        ".resx",
        ".Designer.cs",

        // Web
        ".html",
        ".htm",
        ".css",
        ".scss",
        ".sass",
        ".less",
        ".js",
        ".ts",
        ".jsx",
        ".tsx",
        ".vue",
        ".svelte",
        ".json",
        ".jsonc",
        ".xml",
        ".svg",
        ".xslt",
        ".xsd",
        ".wsdl",

        // Config / Script
        ".yml",
        ".yaml",
        ".toml",
        ".ini",
        ".conf",
        ".cfg",
        ".env",
        ".sh",
        ".bash",
        ".zsh",
        ".ps1",
        ".psm1",
        ".psd1",
        ".cmd",
        ".bat",
        ".sql",
        ".dockerfile",
        ".gitignore",
        ".gitattributes",
        ".editorconfig",
        ".htaccess",
        ".nginx",

        // Documentation
        ".md",
        ".markdown",
        ".txt",
        ".rst",
        ".csv",
        ".tsv",
        ".log",

        // Other code
        ".java",
        ".kt",
        ".groovy",
        ".scala",
        ".py",
        ".rb",
        ".php",
        ".go",
        ".rs",
        ".c",
        ".cpp",
        ".h",
        ".hpp",
        ".swift",
        ".m",
        ".mm",
        ".pl",
        ".pm",
        ".lua",
        ".r",
        ".dart",
    };

    /// <summary>
    /// Checks if a file has a known text file extension.
    /// </summary>
    /// <param name="filePath">The file path to check.</param>
    /// <returns>True if the file has a known text extension; otherwise, false.</returns>
    public static bool IsTextFile(string filePath)
    {
        if (string.IsNullOrWhiteSpace(filePath))
        {
            return false;
        }

        var extension = Path.GetExtension(filePath);
        return KnownExtensions.Contains(extension);
    }
}
