use crate::opencode::models::project::ProjectIndexEntry;
use crate::opencode::parser::db_reader::{
    count_sessions_for_project, open_db, query_projects,
};

pub fn get_projects() -> Result<Vec<ProjectIndexEntry>, String> {
    let conn = match open_db() {
        Ok(c) => c,
        Err(_) => return Ok(vec![]),
    };

    let mut projects = query_projects(&conn);

    for project in &mut projects {
        project.session_count = count_sessions_for_project(&conn, &project.id);
    }

    Ok(projects)
}
