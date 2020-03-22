use std::path::PathBuf;
use std::process::exit;
use pile::Errors;
use structopt::StructOpt;


///Pile – organize your projects from the command-line.
/// 
///Created by Eli Adelhult, and licensed under the MIT license.
#[derive(StructOpt, Debug)]
#[structopt(name = "Pile")]
enum Cli {
    /// Open the documentation in a web browser.
    Doc,

    /// Add a project and create a directory for it
    Add {
        #[structopt()]
        name: String,
        #[structopt(long, env = "HYLLA_WORKSPACE", parse(from_os_str))]
        workspace: PathBuf,
        /// Clone with git
        #[structopt(long, short)]
        clone: Option<String>,
        /// Generate a readme
        #[structopt(long, short)]
        readme: bool,
        #[structopt(
            multiple=true,
            value_name="subject tags"
        )]
        tags: Vec<String>
    },

    /// List all projects
    List {
        /// Filter by project name
        #[structopt(long, short)]
        name: Option<String>,
        /// Filter by tag name
        #[structopt(long, short)]
        tag: Option<String>,
        #[structopt(long, env = "HYLLA_WORKSPACE", parse(from_os_str))]
        workspace: PathBuf
    },

    /// Open the workspace in a file manager
    Workspace {
        #[structopt(long, env = "HYLLA_WORKSPACE", parse(from_os_str))]
        workspace: PathBuf
    },

    /// Print the path of a project directory
    Path {
        #[structopt(
            value_name="PROJECT NAME"
        )]  
        name: String,
        #[structopt(long, env = "HYLLA_WORKSPACE", parse(from_os_str))]
        workspace: PathBuf,
        /// Execute a command in the project path
        #[structopt(
            long,
            short,
            multiple=true,
            value_name="COMMAND ARGS"
        )]
        execute: Option<Vec<String>>
    },

    /// Edit the information about a project
    Edit,

    /// Open a project in a file manager
    Open {
        #[structopt(
            value_name="PROJECT NAME"
        )]  
        name: String, 
        #[structopt(long, env = "HYLLA_WORKSPACE", parse(from_os_str))]
        workspace: PathBuf,
    },

    /// Remove a project from the database
    Remove {
        #[structopt(
            value_name="PROJECT NAME"
        )]  
        name: String, 
        #[structopt(long, env = "HYLLA_WORKSPACE", parse(from_os_str))]
        workspace: PathBuf,
    }
}

fn main() {
    let user_input = Cli::from_args();
    let result = match user_input {
        Cli::Doc        => pile::open_documentation(),
        Cli::Edit       => Err(pile::Errors::NotImplemented),
        Cli::Open {
            name,
            workspace
        }               => pile::open_project(name, workspace),
        Cli::Path {
            name,
            workspace,
            execute
        }               => pile::path_command(name, workspace, execute),
        Cli::List {
            workspace,
            name,
            tag
        }               => pile::print_list(workspace, name, tag),
        Cli::Workspace {
            workspace
        }               => pile::open_workspace(workspace),
        Cli::Remove {
            name,
            workspace
        }               => pile::remove_project(workspace, name),
        Cli::Add {
            name,
            tags,
            workspace,
            clone,
            readme
        }               => pile::add_project(name, tags, workspace, clone, readme),
    };

    match result {
        Ok(_) => (),
        Err(Errors::ProjectNameTaken) => {
            println!("Error: The project named is already in use");
            exit(1);
        },
        Err(Errors::NotImplemented) => {
            println!("Error: This feature is not implemented yet");
            exit(1);
        },
        Err(Errors::DatabaseError) => {
            println!("Error: a database error occurred");
            exit(1);
        },
        Err(Errors::CouldNotGetProject) => {
            println!("Error: could not get project(s)");
            exit(1);
        },
        Err(Errors::FailedToRemoveProject) => {
            println!("Error: could not remove the project from the db");
            exit(1);
        },
        Err(Errors::ProjectDoesNotExist) => {
            println!("Error: Such a project does not exist");
            exit(1);
        },
        Err(Errors::IOError) => {
            println!("Error: an IO error occurred");
            exit(1);
        },
        Err(_) => {
            println!("An error occured");
            exit(1);
        },
    }
}
