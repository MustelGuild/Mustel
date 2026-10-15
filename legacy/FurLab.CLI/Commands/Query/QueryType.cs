namespace FurLab.CLI.Commands.Query;

/// <summary>
/// Types of SQL queries classified by their potential impact.
/// </summary>
public enum QueryType
{
    /// <summary>Safe query that does not modify data or schema.</summary>
    Safe,

    /// <summary>Query that modifies data or schema and requires confirmation.</summary>
    Destructive
}
