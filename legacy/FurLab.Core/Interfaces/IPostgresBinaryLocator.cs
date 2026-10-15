namespace FurLab.Core.Interfaces;

/// <summary>
/// Provides methods to locate PostgreSQL binary executables.
/// </summary>
public interface IPostgresBinaryLocator
{
    /// <summary>
    /// Finds the pg_dump executable in the system PATH or common installation directories.
    /// </summary>
    /// <returns>The full path to pg_dump, or null if not found.</returns>
    string? FindPgDump();

    /// <summary>
    /// Finds the pg_restore executable in the system PATH or common installation directories.
    /// </summary>
    /// <returns>The full path to pg_restore, or null if not found.</returns>
    string? FindPgRestore();

    /// <summary>
    /// Finds the psql executable in the system PATH or common installation directories.
    /// </summary>
    /// <returns>The full path to psql, or null if not found.</returns>
    string? FindPsql();
}
