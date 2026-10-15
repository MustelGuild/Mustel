using System.Text.RegularExpressions;

namespace FurLab.CLI.Commands.Query;

/// <summary>
/// Analyzes SQL queries to detect potentially destructive operations.
/// Uses simple regex-based parsing to identify the first meaningful SQL keyword.
/// </summary>
public static partial class SqlQueryAnalyzer
{
    private static readonly HashSet<string> DestructiveKeywords = new(StringComparer.OrdinalIgnoreCase)
    {
        "INSERT", "UPDATE", "DELETE", "ALTER", "DROP", "CREATE",
        "TRUNCATE", "MERGE", "GRANT", "REVOKE"
    };

    /// <summary>
    /// Regex to strip SQL comments (single-line -- and multi-line /* */).
    /// </summary>
    [GeneratedRegex(@"--.*?$|/\*[\s\S]*?\*/", RegexOptions.Multiline)]
    private static partial Regex SqlCommentRegex();

    /// <summary>
    /// Regex to extract the first SQL keyword from a query.
    /// </summary>
    [GeneratedRegex(@"^\s*(\w+)", RegexOptions.IgnoreCase | RegexOptions.Compiled)]
    private static partial Regex FirstKeywordRegex();

    /// <summary>
    /// Regex to detect SET ROLE pattern.
    /// </summary>
    [GeneratedRegex(@"^\s*SET\s+ROLE", RegexOptions.IgnoreCase | RegexOptions.Compiled)]
    private static partial Regex SetRoleRegex();

    /// <summary>
    /// Analyzes a SQL query and returns its type (Safe or Destructive).
    /// Strips comments and CTEs before identifying the first meaningful keyword.
    /// </summary>
    /// <param name="query">The SQL query to analyze.</param>
    /// <returns>The query type.</returns>
    public static QueryType AnalyzeQuery(string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return QueryType.Safe;
        }

        var cleanedQuery = StripComments(query);
        var firstKeyword = GetFirstKeyword(cleanedQuery);

        if (string.IsNullOrWhiteSpace(firstKeyword))
        {
            return QueryType.Safe;
        }

        if (DestructiveKeywords.Contains(firstKeyword))
        {
            return QueryType.Destructive;
        }

        if (firstKeyword.Equals("SET", StringComparison.OrdinalIgnoreCase))
        {
            if (SetRoleRegex().IsMatch(cleanedQuery))
            {
                return QueryType.Destructive;
            }
        }

        return QueryType.Safe;
    }

    /// <summary>
    /// Gets a human-readable description of the query type.
    /// </summary>
    /// <param name="query">The SQL query.</param>
    /// <returns>Description like "SELECT", "DELETE", "UPDATE", etc.</returns>
    public static string GetQueryTypeDescription(string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return "UNKNOWN";
        }

        var cleanedQuery = StripComments(query);
        var keyword = GetFirstKeyword(cleanedQuery);
        return keyword?.ToUpperInvariant() ?? "UNKNOWN";
    }

    /// <summary>
    /// Strips SQL comments from a query string.
    /// </summary>
    private static string StripComments(string query)
    {
        return SqlCommentRegex().Replace(query, string.Empty);
    }

    /// <summary>
    /// Extracts the first meaningful SQL keyword from a query.
    /// Handles CTEs by looking past WITH ... AS ( to find the actual keyword.
    /// </summary>
    private static string? GetFirstKeyword(string query)
    {
        var match = FirstKeywordRegex().Match(query.Trim());
        if (!match.Success)
        {
            return null;
        }

        var keyword = match.Groups[1].Value;

        if (keyword.Equals("WITH", StringComparison.OrdinalIgnoreCase))
        {
            return ExtractKeywordFromCte(query);
        }

        return keyword;
    }

    /// <summary>
    /// For CTEs (WITH ... AS (...)), finds the first keyword inside the first CTE body.
    /// </summary>
    /// <remarks>
    /// <para>
    /// Known limitation: this parser uses a simple heuristic. It finds the first
    /// "AS (" sequence in the query and inspects the keyword immediately following
    /// the opening parenthesis. This covers the common cases:
    /// <list type="bullet">
    ///   <item><description><c>WITH cte AS (SELECT …) SELECT …</c> → Safe</description></item>
    ///   <item><description><c>WITH cte AS (INSERT …) SELECT …</c> → Destructive</description></item>
    /// </list>
    /// </para>
    /// <para>
    /// Edge cases not handled:
    /// <list type="bullet">
    ///   <item><description>Multiple CTEs: <c>WITH a AS (…), b AS (DELETE …)</c> — only the first CTE body is inspected.</description></item>
    ///   <item><description>Nested CTEs or CTE bodies that don't start with a recognised keyword fall back to <c>"WITH"</c>, which is classified as Safe.</description></item>
    /// </list>
    /// If stricter detection is required, replace this method with a proper SQL parser.
    /// </para>
    /// </remarks>
    private static string ExtractKeywordFromCte(string query)
    {
        var asIndex = query.IndexOf("AS", StringComparison.OrdinalIgnoreCase);
        if (asIndex < 0)
        {
            return "WITH";
        }

        var parenOpen = query.IndexOf('(', asIndex);
        if (parenOpen < 0)
        {
            return "WITH";
        }

        var remaining = query.Substring(parenOpen + 1);
        var innerMatch = FirstKeywordRegex().Match(remaining.Trim());

        if (innerMatch.Success)
        {
            var innerKeyword = innerMatch.Groups[1].Value;

            if (innerKeyword.Equals("SELECT", StringComparison.OrdinalIgnoreCase))
            {
                return "SELECT";
            }

            if (DestructiveKeywords.Contains(innerKeyword))
            {
                return innerKeyword;
            }
        }

        return "WITH";
    }
}
