using FurLab.CLI.Commands.Query;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace FurLab.Tests.Commands;

[TestClass]
public class SqlQueryAnalyzerTests
{
    [TestMethod(DisplayName = "SELECT is classified as Safe")]
    public void AnalyzeQuery_Select_ReturnsSafe()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("SELECT * FROM users"));
    }

    [TestMethod(DisplayName = "SHOW is classified as Safe")]
    public void AnalyzeQuery_Show_ReturnsSafe()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("SHOW tables"));
    }

    [TestMethod(DisplayName = "EXPLAIN is classified as Safe")]
    public void AnalyzeQuery_Explain_ReturnsSafe()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("EXPLAIN SELECT * FROM users"));
    }

    [TestMethod(DisplayName = "INSERT is classified as Destructive")]
    public void AnalyzeQuery_Insert_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("INSERT INTO users (name) VALUES ('test')"));
    }

    [TestMethod(DisplayName = "UPDATE is classified as Destructive")]
    public void AnalyzeQuery_Update_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("UPDATE users SET name = 'test' WHERE id = 1"));
    }

    [TestMethod(DisplayName = "DELETE is classified as Destructive")]
    public void AnalyzeQuery_Delete_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("DELETE FROM users WHERE id = 1"));
    }

    [TestMethod(DisplayName = "ALTER is classified as Destructive")]
    public void AnalyzeQuery_Alter_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("ALTER TABLE users ADD COLUMN email TEXT"));
    }

    [TestMethod(DisplayName = "DROP is classified as Destructive")]
    public void AnalyzeQuery_Drop_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("DROP TABLE users"));
    }

    [TestMethod(DisplayName = "CREATE is classified as Destructive")]
    public void AnalyzeQuery_Create_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("CREATE TABLE test (id INT)"));
    }

    [TestMethod(DisplayName = "TRUNCATE is classified as Destructive")]
    public void AnalyzeQuery_Truncate_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("TRUNCATE TABLE users"));
    }

    [TestMethod(DisplayName = "MERGE is classified as Destructive")]
    public void AnalyzeQuery_Merge_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("MERGE INTO target USING source ON (target.id = source.id) WHEN MATCHED THEN UPDATE SET target.val = source.val"));
    }

    [TestMethod(DisplayName = "GRANT is classified as Destructive")]
    public void AnalyzeQuery_Grant_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("GRANT SELECT ON users TO readonly"));
    }

    [TestMethod(DisplayName = "REVOKE is classified as Destructive")]
    public void AnalyzeQuery_Revoke_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("REVOKE SELECT ON users FROM readonly"));
    }

    [TestMethod(DisplayName = "SET ROLE is classified as Destructive")]
    public void AnalyzeQuery_SetRole_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("SET ROLE admin"));
    }

    [TestMethod(DisplayName = "SET without ROLE is classified as Safe")]
    public void AnalyzeQuery_SetWithoutRole_ReturnsSafe()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("SET search_path TO public"));
    }

    [TestMethod(DisplayName = "Empty query is classified as Safe")]
    public void AnalyzeQuery_Empty_ReturnsSafe()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery(""));
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("   "));
    }

    [TestMethod(DisplayName = "Line comment is ignored before classification")]
    public void AnalyzeQuery_LineComment_IgnoresComment()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("-- This is a comment\nSELECT * FROM users"));
    }

    [TestMethod(DisplayName = "Block comment is ignored before classification")]
    public void AnalyzeQuery_BlockComment_IgnoresComment()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("/* block comment */ SELECT * FROM users"));
    }

    [TestMethod(DisplayName = "Comment before DELETE does not prevent detection")]
    public void AnalyzeQuery_CommentBeforeDelete_StillDetectsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("-- careful!\nDELETE FROM users"));
    }

    [TestMethod(DisplayName = "CTE with internal SELECT is classified as Safe")]
    public void AnalyzeQuery_CteWithSelect_ReturnsSafe()
    {
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("WITH temp AS (SELECT * FROM users) SELECT * FROM temp"));
    }

    [TestMethod(DisplayName = "CTE with internal INSERT is classified as Destructive")]
    public void AnalyzeQuery_CteWithInsert_ReturnsDestructive()
    {
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery("WITH temp AS (INSERT INTO users VALUES (1)) SELECT * FROM temp"));
    }

    [TestMethod(DisplayName = "GetQueryTypeDescription returns uppercase keyword")]
    public void GetQueryTypeDescription_ReturnsUppercaseKeyword()
    {
        Assert.AreEqual("SELECT", SqlQueryAnalyzer.GetQueryTypeDescription("select * from users"));
        Assert.AreEqual("DELETE", SqlQueryAnalyzer.GetQueryTypeDescription("delete from users"));
    }

    [TestMethod(DisplayName = "GetQueryTypeDescription returns UNKNOWN for empty query")]
    public void GetQueryTypeDescription_Empty_ReturnsUnknown()
    {
        Assert.AreEqual("UNKNOWN", SqlQueryAnalyzer.GetQueryTypeDescription(""));
    }

    // ── CTE edge cases (documented limitations) ──────────────────────────────

    [TestMethod(DisplayName = "Multiple CTEs: second destructive CTE — known limitation returns Safe")]
    public void AnalyzeQuery_MultipleCtes_SecondDestructive_ReturnsSafe_KnownLimitation()
    {
        // KNOWN LIMITATION: only the first CTE body is inspected.
        // WITH a AS (SELECT …), b AS (DELETE …) → the parser sees SELECT in the first
        // CTE body and returns Safe, even though the second CTE is destructive.
        var query = "WITH a AS (SELECT 1), b AS (DELETE FROM users) SELECT * FROM a";
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery(query));
    }

    [TestMethod(DisplayName = "Multiple CTEs: first destructive CTE is correctly detected")]
    public void AnalyzeQuery_MultipleCtes_FirstDestructive_ReturnsDestructive()
    {
        var query = "WITH a AS (DELETE FROM users WHERE id = 1), b AS (SELECT 1) SELECT * FROM b";
        Assert.AreEqual(QueryType.Destructive, SqlQueryAnalyzer.AnalyzeQuery(query));
    }

    [TestMethod(DisplayName = "CTE without AS returns Safe (fallback for WITH)")]
    public void AnalyzeQuery_CteWithoutAs_ReturnsSafe()
    {
        // Malformed CTE — treated as Safe (unknown keyword fallback)
        Assert.AreEqual(QueryType.Safe, SqlQueryAnalyzer.AnalyzeQuery("WITH cte SELECT * FROM cte"));
    }
}
