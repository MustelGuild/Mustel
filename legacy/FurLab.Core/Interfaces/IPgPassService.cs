using FurLab.Core.Models;

namespace FurLab.Core.Interfaces;

/// <summary>
/// Contract for the pgpass.conf file management service.
/// </summary>
public interface IPgPassService
{
    /// <summary>
    /// Adds a new entry to pgpass.conf.
    /// Creates the directory and file if necessary.
    /// Detects duplicates and returns <see cref="PgPassResult.Duplicate"/> without modifying the file.
    /// </summary>
    /// <param name="entry">Entry to add. Password can never be empty.</param>
    /// <param name="filePath">Full path to pgpass.conf. If null, uses the default Windows path.</param>
    /// <returns>Operation result.</returns>
    PgPassResult AddEntry(PgPassEntry entry, string? filePath = null);

    /// <summary>
    /// Lists all entries from pgpass.conf.
    /// Returns an empty enumerable if the file does not exist or is empty.
    /// Comment lines (starting with '#') are ignored.
    /// </summary>
    /// <param name="filePath">Full path to pgpass.conf. If null, uses the default Windows path.</param>
    /// <returns>Enumerable of parsed entries.</returns>
    IEnumerable<PgPassEntry> ListEntries(string? filePath = null);

    /// <summary>
    /// Removes the entry identified by the key (Hostname, Port, Database, Username).
    /// If the entry is not found, returns an informative result without modifying the file.
    /// </summary>
    /// <param name="key">Entry containing the identity key to locate and remove.</param>
    /// <param name="filePath">Full path to pgpass.conf. If null, uses the default Windows path.</param>
    /// <returns>Operation result.</returns>
    PgPassResult RemoveEntry(PgPassEntry key, string? filePath = null);
}
