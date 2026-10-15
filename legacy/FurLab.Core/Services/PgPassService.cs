using FurLab.Core.Interfaces;
using FurLab.Core.Models;

using Microsoft.Extensions.Logging;

namespace FurLab.Core.Services;

/// <summary>
/// Service for managing the pgpass.conf file on Windows.
/// Implements AddEntry, ListEntries, and RemoveEntry for US1, US2, and US3.
/// </summary>
public class PgPassService(ILogger<PgPassService>? logger = null) : IPgPassService
{
    private readonly ILogger<PgPassService>? _logger = logger;

    // =========================================================================
    // US1 — AddEntry
    // =========================================================================

    /// <inheritdoc />
    public PgPassResult AddEntry(PgPassEntry entry, string? filePath = null)
    {
        filePath ??= ResolveDefaultPath();

        // Validation: password cannot be empty (RF-011)
        if (string.IsNullOrEmpty(entry.Password))
        {
            return PgPassResult.Fail("Error: password cannot be empty.");
        }

        try
        {
            // Ensure directory exists (RF-002)
            var directory = Path.GetDirectoryName(filePath)!;
            if (!Directory.Exists(directory))
            {
                Directory.CreateDirectory(directory);
                _logger?.LogInformation("Directory created: {Directory}", directory);
            }

            // Read existing entries to check for duplicates (RF-006)
            var existingEntries = ReadAllEntries(filePath).ToList();
            if (existingEntries.Any(e => e.IdentityKey == entry.IdentityKey))
            {
                var key = FormatKey(entry);
                _logger?.LogInformation("Entry already exists: {Key}", key);
                return PgPassResult.Duplicate($"Entry already exists: {key}");
            }

            // Serialize and append to the file (RF-003, RF-010)
            var line = SerializeEntry(entry);
            File.AppendAllText(filePath, line + Environment.NewLine);

            var addedKey = FormatKey(entry);
            _logger?.LogInformation("Entry added: {Key}", addedKey);
            return PgPassResult.Ok($"Entry added: {addedKey}");
        }
        catch (UnauthorizedAccessException ex)
        {
            // RF-012: permission denied
            _logger?.LogError(ex, "Permission denied while accessing {Path}", filePath);
            return PgPassResult.Fail(
                $"Error: no permission to write to {Path.GetDirectoryName(filePath)}. " +
                "Run the command in a terminal with administrator privileges.");
        }
        catch (IOException ex)
        {
            // RF-013: read-only or locked file
            _logger?.LogError(ex, "I/O error while writing to {Path}", filePath);
            return PgPassResult.Fail(
                "Error: could not write to pgpass.conf — the file may be " +
                "read-only or in use by another process.");
        }
    }

    // =========================================================================
    // US2 — ListEntries
    // =========================================================================

    /// <inheritdoc />
    public IEnumerable<PgPassEntry> ListEntries(string? filePath = null)
    {
        filePath ??= ResolveDefaultPath();
        return ReadAllEntries(filePath);
    }

    // =========================================================================
    // US3 — RemoveEntry
    // =========================================================================

    /// <inheritdoc />
    public PgPassResult RemoveEntry(PgPassEntry key, string? filePath = null)
    {
        filePath ??= ResolveDefaultPath();

        // File does not exist: nothing to remove
        if (!File.Exists(filePath))
        {
            var keyStr = FormatKey(key);
            return PgPassResult.Fail($"Entry not found: {keyStr}");
        }

        try
        {
            var originalLines = File.ReadAllLines(filePath);
            var filteredLines = new List<string>();
            var found = false;

            foreach (var line in originalLines)
            {
                if (line.StartsWith('#') || string.IsNullOrWhiteSpace(line))
                {
                    // Preserve comments and blank lines
                    filteredLines.Add(line);
                    continue;
                }

                var entry = ParseLine(line);
                if (entry != null && entry.IdentityKey == key.IdentityKey)
                {
                    found = true;
                    // Do not add the removed line
                }
                else
                {
                    filteredLines.Add(line);
                }
            }

            if (!found)
            {
                var keyStr = FormatKey(key);
                return PgPassResult.Fail($"Entry not found: {keyStr}");
            }

            File.WriteAllLines(filePath, filteredLines);
            var removedKey = FormatKey(key);
            _logger?.LogInformation("Entry removed: {Key}", removedKey);
            return PgPassResult.Ok($"Entry removed: {removedKey}");
        }
        catch (UnauthorizedAccessException ex)
        {
            _logger?.LogError(ex, "Permission denied while accessing {Path}", filePath);
            return PgPassResult.Fail(
                $"Error: no permission to write to {Path.GetDirectoryName(filePath)}. " +
                "Run the command in a terminal with administrator privileges.");
        }
        catch (IOException ex)
        {
            _logger?.LogError(ex, "I/O error while writing to {Path}", filePath);
            return PgPassResult.Fail(
                "Error: could not write to pgpass.conf — the file may be " +
                "read-only or in use by another process.");
        }
    }

    // =========================================================================
    // Private methods — serialization, parse, escape, path
    // =========================================================================

    /// <summary>
    /// Resolves the default pgpass.conf path on Windows.
    /// %APPDATA%\postgresql\pgpass.conf
    /// </summary>
    private static string ResolveDefaultPath()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "postgresql", "pgpass.conf");
    }

    /// <summary>
    /// Reads all valid entries from the file. Returns empty if the file does not exist.
    /// Ignores comment lines (starting with '#') and blank lines.
    /// </summary>
    private static IEnumerable<PgPassEntry> ReadAllEntries(string filePath)
    {
        if (!File.Exists(filePath))
        {
            yield break;
        }

        foreach (var line in File.ReadAllLines(filePath))
        {
            if (string.IsNullOrWhiteSpace(line) || line.StartsWith('#'))
            {
                continue;
            }

            var entry = ParseLine(line);
            if (entry != null)
            {
                yield return entry;
            }
        }
    }

    /// <summary>
    /// Serializes a <see cref="PgPassEntry"/> to the pgpass format.
    /// Applies escaping to the password before writing: ':' → '\:' and '\' → '\\'
    /// </summary>
    internal static string SerializeEntry(PgPassEntry entry)
    {
        var escapedPassword = EscapePassword(entry.Password);
        return $"{entry.Hostname}:{entry.Port}:{entry.Database}:{entry.Username}:{escapedPassword}";
    }

    /// <summary>
    /// Parses a pgpass line into a <see cref="PgPassEntry"/>.
    /// Applies unescaping to the password: '\:' → ':' and '\\' → '\'
    /// Returns null if the line is invalid (fewer than 5 fields).
    /// </summary>
    internal static PgPassEntry? ParseLine(string line)
    {
        // The pgpass format uses ':' as a separator but ':' can be escaped as '\:'
        // We need to split only by unescaped ':'
        var fields = SplitEscaped(line);
        if (fields.Count < 5)
        {
            return null;
        }

        return new PgPassEntry
        {
            Hostname = fields[0],
            Port = fields[1],
            Database = fields[2],
            Username = fields[3],
            Password = UnescapePassword(string.Join(":", fields.Skip(4)))
        };
    }

    /// <summary>
    /// Splits a pgpass line by unescaped ':'.
    /// Respects the '\:' escape to not split at the escaped separator.
    /// </summary>
    private static List<string> SplitEscaped(string line)
    {
        var fields = new List<string>();
        var currentField = new System.Text.StringBuilder();

        for (var i = 0; i < line.Length; i++)
        {
            if (line[i] == '\\' && i + 1 < line.Length)
            {
                // Escape sequence: consume both characters
                currentField.Append(line[i]);
                currentField.Append(line[i + 1]);
                i++;
            }
            else if (line[i] == ':')
            {
                fields.Add(currentField.ToString());
                currentField.Clear();
            }
            else
            {
                currentField.Append(line[i]);
            }
        }

        fields.Add(currentField.ToString());
        return fields;
    }

    /// <summary>
    /// Applies escaping to the password according to the pgpass specification:
    /// '\' → '\\' (must be done BEFORE escaping ':')
    /// ':' → '\:'
    /// </summary>
    internal static string EscapePassword(string password)
    {
        // Order matters: escape '\' first to avoid re-escaping '\:'
        return password
            .Replace(@"\", @"\\")
            .Replace(":", @"\:");
    }

    /// <summary>
    /// Removes escaping from the password read from the pgpass file:
    /// '\:' → ':'
    /// '\\' → '\'
    /// </summary>
    internal static string UnescapePassword(string escaped)
    {
        // Process character by character for correct order
        var result = new System.Text.StringBuilder();
        for (var i = 0; i < escaped.Length; i++)
        {
            if (escaped[i] == '\\' && i + 1 < escaped.Length)
            {
                var next = escaped[i + 1];
                if (next == ':')
                {
                    result.Append(':');
                    i++;
                }
                else if (next == '\\')
                {
                    result.Append('\\');
                    i++;
                }
                else
                {
                    result.Append(escaped[i]);
                }
            }
            else
            {
                result.Append(escaped[i]);
            }
        }

        return result.ToString();
    }

    /// <summary>
    /// Formats an entry's identity key for display.
    /// Example: "localhost:5432:my_db:postgres"
    /// </summary>
    private static string FormatKey(PgPassEntry entry)
        => $"{entry.Hostname}:{entry.Port}:{entry.Database}:{entry.Username}";
}
