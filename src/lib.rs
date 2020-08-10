use std::fs;
use std::io;
use std::process::Output;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use clipboard::ClipboardProvider;
use clipboard::ClipboardContext;
use rusqlite::{Connection, params};
use rusqlite::NO_PARAMS;
use prettytable::{Table, Row, Cell};
use prettytable::format;
use open;

// Todo:
// * migrate
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

    if projects.is_empty(){
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
/// This should def be changed.
/// Returns the path to a specific project.
pub fn get_project_path(name: String, workspace: &PathBuf) -> Result<PathBuf, Errors> {
    let conn = get_connection(&workspace)?;
    let project = Project::get_from_db_by_name(&name, &conn)?;
    Ok(project.get_path(&workspace))
}

/// Prints the path to a given project.
pub fn path_command(
    name: String,
    workspace: PathBuf,
    clipboard: bool,
    execute: Option<Vec<String>>
    ) -> Result<(), Errors> {
    let path = get_project_path(name, &workspace)?;
    let path_string = path.to_string_lossy();
    println!("{}", path_string);

    if clipboard {
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        ctx.set_contents(path_string.to_owned().to_string()).unwrap();
        println!("The path has been copied to the clipboard.");
    }
    // If the user specified a command, execute it.
    if let Some(args) = execute{
        if !args.is_empty() {
            let output = execute_command(args, &path);
            if output.is_err() {
                println!("Failed to execute the command.");
            } else if let Ok(value) = output {
                io::stdout().write_all(&value.stdout).unwrap();
                io::stderr().write_all(&value.stderr).unwrap();
            }
        }
    }
 
    Ok(())
}

/// Helper function that will execute one or more commands
fn execute_command(args: Vec<String>, path: &PathBuf) -> Result<Output, io::Error> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .current_dir(path)
            .arg("/C")
            .args(&args)
            .output()
    } else {
        Command::new("sh")
            .current_dir(path)
            .arg("-c")
            .args(&args)
            .output()
    }
}

/// Opens the path to a project in a file browser.
pub fn open_project(name: String, workspace: PathBuf) -> Result<(), Errors> {
    let path = get_project_path(name, &workspace)?;
    open::that(path)?;
    Ok(())
}

pub fn edit(
    name: String,
    new_name: Option<String>,
    new_tags: Option<Vec<String>>,
    workspace: PathBuf
    ) -> Result<(), Errors> {
    
    let conn = get_connection(&workspace)?;
    let mut project = Project::get_from_db_by_name(&name, &conn)?;

    if let Some(name) = new_name {
        let returned_name = project.edit_name(&name, &conn, &workspace)?;
        println!("The name has been changed to {}", returned_name);
    }

    if let Some(tags) = new_tags {
        project.edit_tags(&tags, &conn)?;
        println!("The tags has been changed to {}", tags.join(", "));
    }

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

    let project = Project::new(name, tags);
    let conn = get_connection(&workspace)?;

    if Project::name_taken(&project.name, &conn) {
        return Err(Errors::ProjectNameTaken);
    }

    project.create_directory(&workspace)?;
    project.add_to_db(&conn)?;

    if readme {
        let mut readme_path = project.get_path(&workspace);
        readme_path.push("README.md");

        let mut file = fs::File::create(&readme_path)?;

        let file_content = format!("# {}", &project.name);
        file.write_all(file_content.as_bytes())?;
    }
    
    if let Some(clone_url) = clone {
        Command::new("git")
             .current_dir(&project.get_path(&workspace))
             .args(vec!["clone", &clone_url, "."])
             .output()?;
    }
    
    println!("Project created");
    println!("{}", project.get_path(&workspace).to_string_lossy());
    Ok(())
}  

#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub tags: Vec<String>,
}

impl Project {
    /// Creates a new Project
    pub fn new(name: String, tags: Vec<String>) -> Self{
        let cleaned_name = name.trim().replace(" ", "-");
        Project {
            name: cleaned_name,
            tags
        }
    }

    pub fn get_path(&self, workspace: &PathBuf) -> PathBuf {
        let mut path = workspace.clone();
        path.push(&self.name);
        path
    }

    /// Checks if a project name is already in use
    /// TODO: Check if a conflicting directory exists.
    pub fn name_taken(name: &str, conn: &Connection) -> bool {
        let mut stmt = conn.prepare("SELECT id FROM projects WHERE name = ?1").unwrap();
        stmt.exists(params![name]).expect("Could not SELECT from database")
    }
    
    /// Returns a single Project based on the provided name
    /// **TODO:** this function should return a Result instead of panic if it fails.
    pub fn get_from_db_by_name(name:&str, conn: &Connection) -> Result<Project, Errors> {
        let mut stmt = conn.prepare("SELECT tags FROM projects WHERE name = ?1")
            .unwrap();
        let mut db_output = stmt.query_map(params![name], |row| {
            let tags_string: String = row.get(0)?;
            Ok(Project {
                name: String::from(name),
                tags: tags_string
                    .split(',')
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
    pub fn remove_from_db_by_name(name: &str, conn: &Connection) -> Result<(), Errors>{
        let mut stmt = conn.prepare("DELETE FROM projects WHERE name = ?1").unwrap();
        match stmt.execute(params![name]) {
            Ok(_) => Ok(()),
            Err(_) => Err(Errors::FailedToRemoveProject)
        }
    }

    /// Edits the name of a project, the cleaned new name is returned on Ok()
    pub fn edit_name(&mut self, new_name: &str, conn:&Connection, workspace: &PathBuf) -> Result<String, Errors>{
        let cleaned_name = new_name.trim().replace(" ", "-");

        let mut stmt = conn.prepare(
            "UPDATE projects SET name = ?1 WHERE name = ?2"
        ).unwrap();
        stmt.execute(params![cleaned_name, self.name])?;

        let mut new_path = workspace.clone();
        new_path.push(&cleaned_name);

        fs::rename(self.get_path(&workspace), &new_path)?;

        self.name = cleaned_name.clone();

        Ok(cleaned_name)
    }

    pub fn edit_tags(
        &mut self,
        new_tags: &[String],
        conn:&Connection,
    ) -> Result<(), Errors> {

        let mut stmt = conn.prepare(
            "UPDATE projects SET tags = ?1 WHERE name = ?2"
        ).unwrap();

        stmt.execute(params![&new_tags.join(","), self.name])?;

        self.tags = new_tags.to_owned();
        Ok(())
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
        ) -> Result<Vec<Project>, Errors> {
        
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
                        "SELECT tags, name
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
                        "SELECT tags, name
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
                        "SELECT tags, name
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
                        "SELECT tags, name
                        FROM projects
                        ORDER BY name COLLATE NOCASE ASC"
                    ).unwrap(),
                    FilterBy::None
                )
        };

        // This closure turns a rusqlite row into an actual Project struct,
        let from_row_to_project = |row: &rusqlite::Row| -> rusqlite::Result<Project>{
            let tags_string: String = row.get(0)?;
            Ok(Project {
                name: row.get(1)?,
                tags: tags_string
                    .split(',')
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
            "INSERT INTO projects (name, tags) VALUES (?1, ?2)",
            params![self.name, self.tags.join(",")]
        )?;
        Ok(())
    }

    /// Create a directory for the project.
    pub fn create_directory(&self, workspace: &PathBuf) -> std::io::Result<()>{
        fs::create_dir(&self.get_path(&workspace))?;
        Ok(())
    }
}