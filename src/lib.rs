use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use rusqlite::{Connection, params};
use rusqlite::NO_PARAMS;
use prettytable::{Table, Row, Cell};
use prettytable::format;
use open;

// Todo:
// * migrate
// * edit
// * keep track from where things are git cloned
//      + create a fetch command
// * better errors, when a conflicting dir exists for instance

/// Enum of all the possible Errors
pub enum Errors {
    ProjectNameTaken,
    DirAlreadyExists,
    IOError,
    NotImplemented,
    DatabaseError,
    CouldNotGetProject,
    ProjectDoesNotExist,
    FailedToRemoveProject
}

// convert IO Errors to the type Errors
impl From<std::io::Error> for Errors {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Errors::DirAlreadyExists
        } else {
            Errors::IOError
        }
    }
}

// convert IO Errors to the type Errors
impl From<rusqlite::Error> for Errors {
    fn from(_: rusqlite::Error) -> Self {
        Errors::DatabaseError
    }
}


/// Opens the documentation/github in a browser window.
pub fn open_documentation() -> Result<(), Errors> {
    let url = "https://github.com/adelhult/pile";
    println!("Documentation can be found at: {}", url);
    open::that(url)?;
    Ok(())
}

/// Opens the workspace path in a file manager.
/// Explorer or Finder for instance.
pub fn open_workspace(workspace: PathBuf) -> Result<(), Errors> {
    open::that(&workspace)?;
    Ok(())
}

/// Prints a list of all the projects
pub fn print_list(
    workspace: PathBuf,
    name: Option<String>,
    tag: Option<String>
    ) -> Result<(), Errors> {

    let conn = get_connection(&workspace)?;
    let projects = Project::fetch_from_db(&conn, name, tag)?;

    if projects.len() < 1 {
        println!("No projects where found :(");
        return Ok(());
    }

    // Create a table
    let mut table = Table::new();
    table.set_titles(Row::new(vec![Cell::new("Project name"), Cell::new("Tags")]));
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    for project in projects.iter() {
        table.add_row(Row::new(vec![
            Cell::new(&project.name),
            Cell::new(&project.tags.join(", ")
        )]));
    }
    table.printstd();

    Ok(())
}

/// Removes a project from the database.
pub fn remove_project(workspace: PathBuf, name: String) -> Result<(), Errors> {
    let conn = get_connection(&workspace)?;

    // Check if the project name actually exists
    if !Project::name_taken(&name, &conn) {
        return Err(Errors::ProjectDoesNotExist);
    }

    Project::remove_from_db_by_name(&name, &conn)?;
    println!("The project \"{}\" was removed from the database", name);
    println!("Note: the actual directory has not been removed");
    Ok(())
}

/// Returns the path to a specific project.
pub fn get_project_path(name: String, workspace: &PathBuf) -> Result<PathBuf, Errors> {
    let conn = get_connection(workspace)?;
    let project = Project::get_from_db_by_name(&name, &conn)?;
    Ok(project.path)
}

/// Prints the path to a given project.
pub fn path_command(name: String, workspace: PathBuf, execute: Option<Vec<String>>) -> Result<(), Errors> {
    let path = get_project_path(name, &workspace)?;
    println!("{}", path.to_string_lossy());

    // If the user specified a command, execute it.
    match execute {
        None => (),
        Some(args) =>  {
            if args.len() > 0 {
                Command::new(&args[0])
                    .current_dir(&path)
                    .args(&args[1..])
                    .spawn()?;
            }
        }
    }
    Ok(())
}

/// Opens the path to a project in a file browser.
pub fn open_project(name: String, workspace: PathBuf) -> Result<(), Errors> {
    let path = get_project_path(name, &workspace)?;
    open::that(path)?;
    Ok(())
}

/// Gets a connection to the database
/// If a file named "pile.db" does not
/// exist in the workspace directory, such a file will be created.
/// 
/// # Example:
/// ``` 
/// let conn = get_connection(&workspace)
///     .expect("Failed to connect to the database");
/// ``` 
pub fn get_connection(workspace: &PathBuf) -> Result<Connection, rusqlite::Error> {
    let mut filepath = workspace.clone();
    filepath.push("pile.db");
    let conn = Connection::open(filepath)?;
    conn.execute(
        "create table if not exists projects (
             id integer primary key,
             name text not null unique,
             path text not null unique,
             tags text
         )",
        NO_PARAMS,
    )?;
    Ok(conn)
}

/// Creates a new instance of a Project struct
/// and adds it to the database, and creates a directory.
pub fn add_project(
    name: String,
    tags: Vec<String>,
    workspace:PathBuf,
    clone: Option<String>,
    readme: bool
    ) -> Result<(), Errors> {

    let project = Project::new(name, tags, &workspace);
    let conn = get_connection(&workspace).expect("Failed to connect to the database.");

    if Project::name_taken(&project.name, &conn) {
        return Err(Errors::ProjectNameTaken);
    }

    project.create_directory()?;
    project
        .add_to_db(&conn)
        .expect("Failed to add the project to the database.");

    if readme {
        let mut readme_path = project.path.clone();
        readme_path.push("README.md");

        let mut file = fs::File::create(&readme_path)?;

        let file_content = format!("# {}", &project.name);
        file.write_all(file_content.as_bytes())?;
    }
    
    if clone.is_some() {
         let clone_url = clone.unwrap();
         Command::new("git")
             .current_dir(&project.path)
             .args(vec!["clone", &clone_url, "."])
             .output()?;
    }
    
    println!("Project created");
    println!("{}", project.path.to_string_lossy());
    Ok(())
}  

#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub tags: Vec<String>,
}

impl Project {
    /// Creates a new Project
    pub fn new(name: String, tags: Vec<String>, workspace: &PathBuf) -> Self{
        let cleaned_name = name.trim().replace(" ", "-");
        let mut generated_path = workspace.clone();
        generated_path.push(&cleaned_name);
        Project {
            name: cleaned_name,
            tags: tags,
            path: generated_path,
        }
    }

    /// Checks if a project name is already in use
    /// TODO: Check if a conflicting directory exists.
    pub fn name_taken(name: &String, conn: &Connection) -> bool {
        let mut stmt = conn.prepare("SELECT id FROM projects WHERE name = ?1").unwrap();
        let answer = stmt.exists(params![name]).expect("Could not SELECT from database");
        answer
    }
    
    /// Returns a single Project based on the provided name
    /// **TODO:** this function should return a Result instead of panic if it fails.
    pub fn get_from_db_by_name(name:&String, conn: &Connection) -> Result<Project, Errors> {
        let mut stmt = conn.prepare("SELECT path, tags FROM projects WHERE name = ?1")
            .unwrap();
        let mut db_output = stmt.query_map(params![name], |row| {
            let path_string: String = row.get(0)?;
            let tags_string: String = row.get(1)?;
            Ok(Project {
                name: name.clone(),
                path: PathBuf::from(path_string),
                tags: tags_string
                    .split(",")
                    .map(|tag| tag.to_string())
                    .filter(|tag| tag != "")
                    .collect()
            })
        }).unwrap();

        match db_output.nth(0) {
            Some(project) => Ok(project.unwrap()),
            None => Err(Errors::CouldNotGetProject)
        }
    }

    /// Remove a project from the database (based on its name)
    /// **This method is not completed yet.**
    pub fn remove_from_db_by_name(name: &String, conn: &Connection) -> Result<(), Errors>{
        let mut stmt = conn.prepare("DELETE FROM projects WHERE name = ?1").unwrap();
        match stmt.execute(params![name]) {
            Ok(_) => Ok(()),
            Err(_) => Err(Errors::FailedToRemoveProject)
        }
    }

    /// Get multiple projects from the database.
    /// The name_query and tag_query is used to filter out results
    /// based on project name or a subject tag name.
    /// # Example:
    /// ``` 
    /// let projects = Project::fetch_from_db(&conn, None, Some(String::from("python"))).unwrap();
    /// ``` 
    pub fn fetch_from_db(
        conn: &Connection,
        name_query: Option<String>,
        tag_query: Option<String>
        ) -> Result<Vec<Project>, Errors>{

        enum FilterBy {
            NameAndTag {name: String, tag: String},
            Name{name: String},
            Tag{tag: String},
            None
        }
        // Prepere the correct sqlite query:
        let (mut stmt, filter_by) = match (name_query, tag_query) {

            // If the user wants to filter by both project name and tag
            (Some(name), Some(tag)) => 
                (
                    conn.prepare(
                        "SELECT path, tags, name
                        FROM projects
                        WHERE name LIKE ?1
                        AND tags LIKE ?2
                        ORDER BY name COLLATE NOCASE ASC"
                    ).unwrap(),
                    FilterBy::NameAndTag {name, tag}
                ),

            // If the user wants to filter by project name
            (Some(name), None) => 
                (
                    conn.prepare(
                        "SELECT path, tags, name
                        FROM projects
                        WHERE name LIKE ?1
                        ORDER BY name COLLATE NOCASE ASC"
                    ).unwrap(),
                    //params!(format!("%{}%", name))
                    FilterBy::Name{name}
                ),

            // If the user wants to filter by tag
            (None, Some(tag)) => 
                (
                    conn.prepare(
                        "SELECT path, tags, name
                        FROM projects
                        WHERE tags LIKE ?1
                        ORDER BY name COLLATE NOCASE ASC"
                    ).unwrap(),
                    //params!(format!("%{}%", tags))
                    FilterBy::Tag {tag}
                ),

            // If the user does not want to filter = get everything
            _ => 
                (
                    conn.prepare(
                        "SELECT path, tags, name
                        FROM projects
                        ORDER BY name COLLATE NOCASE ASC"
                    ).unwrap(),
                    FilterBy::None
                )
        };

        // This closure turns a rusqlite row into an actual Project struct,
        let from_row_to_project = |row: &rusqlite::Row| -> rusqlite::Result<Project>{
            let path_string: String = row.get(0)?;
            let tags_string: String = row.get(1)?;
            Ok(Project {
                name: row.get(2)?,
                path: PathBuf::from(path_string),
                tags: tags_string
                    .split(",")
                    .map(|tag| tag.to_string())
                    .filter(|tag| tag != "")
                    .collect()
            })
        };

        // Get the actual projects from the database
        let db_output = match filter_by {
            FilterBy::None => 
                stmt.query_map(NO_PARAMS, from_row_to_project),

            FilterBy::NameAndTag {name, tag} => 
                stmt.query_map(
                    params![format!("%{}%", name), format!("%{}%", tag)],
                    from_row_to_project
                ),

            FilterBy::Name {name} => 
                stmt.query_map(
                    params![format!("%{}%", name)],
                    from_row_to_project
                ),

            FilterBy::Tag {tag} => 
                stmt.query_map(
                    params![format!("%{}%", tag)],
                    from_row_to_project
            )
        };

        match db_output {
            Err(_) => Err(Errors::CouldNotGetProject),
            Ok(projects) => Ok(projects.map(|project| project.unwrap()).collect())
        }
    }

    /// Adds the project itself to a database using the given Connection.
    pub fn add_to_db(&self, conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "INSERT INTO projects (name, path, tags) VALUES (?1, ?2, ?3)",
            params![self.name, self.path.to_string_lossy(), self.tags.join(",")]
        )?;
        Ok(())
    }

    /// Create a directory for the project.
    pub fn create_directory(&self) -> std::io::Result<()>{
        fs::create_dir(&self.path)?;
        Ok(())
    }
}