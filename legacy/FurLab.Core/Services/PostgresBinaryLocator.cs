using FurLab.Core.Interfaces;

namespace FurLab.Core.Services;

/// <summary>
/// Provides methods to locate PostgreSQL binary executables.
/// </summary>
public class PostgresBinaryLocator : IPostgresBinaryLocator
{
    /// <inheritdoc/>
    public string? FindPgDump() => FindExecutable("pg_dump");

    /// <inheritdoc/>
    public string? FindPgRestore() => FindExecutable("pg_restore");

    /// <inheritdoc/>
    public string? FindPsql() => FindExecutable("psql");

    private static string? FindExecutable(string executableName)
    {
        var paths = Environment.GetEnvironmentVariable("PATH")?.Split(Path.PathSeparator) ?? [];

        foreach (var path in paths)
        {
            var exePath = Path.Combine(path, $"{executableName}.exe");
            if (File.Exists(exePath))
            {
                return exePath;
            }

            exePath = Path.Combine(path, executableName);
            if (File.Exists(exePath))
            {
                return exePath;
            }
        }

        var commonPaths = new[]
        {
            @"C:\Program Files\PostgreSQL",
            @"C:\PostgreSQL"
        };

        foreach (var basePath in commonPaths)
        {
            if (Directory.Exists(basePath))
            {
                foreach (var versionDir in Directory.GetDirectories(basePath))
                {
                    var exePath = Path.Combine(versionDir, "bin", $"{executableName}.exe");
                    if (File.Exists(exePath))
                    {
                        return exePath;
                    }
                }
            }
        }

        var unixPaths = new[]
        {
            "/usr/bin",
            "/usr/local/bin",
            "/usr/local/pgsql/bin"
        };

        foreach (var unixPath in unixPaths)
        {
            if (Directory.Exists(unixPath))
            {
                var exePath = Path.Combine(unixPath, executableName);
                if (File.Exists(exePath))
                {
                    return exePath;
                }
            }
        }

        return null;
    }
}
