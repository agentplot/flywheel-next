## Purpose

Bringing an organization and its first host into existence deterministically:
the two central repositories, the connection to the git host, and a layout on
disk no person makes by hand.

## ADDED Requirements

### Requirement: Initialization is an object with a machine, driven repeatably

Initialization SHALL be the machinery's, deterministic and repeatable (204). An
organization SHALL be an object with a machine of its own, and the
initialization command SHALL drive it: create or adopt the blueprints repository
from the blueprints template, create the state repository with the profile's
layout, record that the organization's GitHub App must be installed, and
register the first host (204). Every step SHALL be an effect with a proof, so
running it again changes nothing, and the reconciler that advances work SHALL
advance a half-finished bootstrap (204).

#### Scenario: Running initialization twice
- **WHEN** initialization is run again against an organization already
  bootstrapped
- **THEN** every step finds its proof present, nothing is created or written,
  and the organization's state is unchanged (204)

#### Scenario: A half-finished bootstrap is advanced, not restarted
- **WHEN** initialization stops after the blueprints repository exists and
  before the state repository does
- **THEN** the next tick creates the state repository and continues from there,
  with no step repeated (204)

#### Scenario: The App's installation waits on the operator
- **WHEN** the machinery has recorded that the App must be installed and the
  installation is not yet present
- **THEN** the organization stands at a decision under attention until the
  installation is seen, and no agent performs it (204, 207, 82)

### Requirement: A host joins by one command and never by hand

A host SHALL join by one command (205). It SHALL clone the state, the blueprints
and every tracked built repository as bare repositories under one root the
manifest names, keep one checkout of each shared line for the machinery's own
merges (205), and make no worktree while the line-and-place effects are
recorded (93a). The layout on disk SHALL be the profile's, and a host that finds
a hand-made layout SHALL refuse to start and SHALL say what differs (205).

#### Scenario: Joining clones what the manifest names
- **WHEN** the join command is run for an organization
- **THEN** the state, blueprints and tracked built repositories are cloned bare
  under the manifest's root, each shared line is checked out once, and no other
  worktree exists (205)

#### Scenario: A hand-made layout is refused
- **WHEN** a host starts and finds a path under its root that differs from the
  profile's layout
- **THEN** it refuses to start, names the first path that differs and what was
  expected, and takes nothing (205)

#### Scenario: A repeated join creates nothing twice
- **WHEN** the join command is run again on a host already joined
- **THEN** it clones only what is missing and re-runs the layout check (205)

### Requirement: A host has one address with the organization in the path

A host SHALL have one address, and the organization SHALL be in the path; a host
serving several organizations SHALL serve them all at that one address, and a
link SHALL name the organization it opens (205a).

#### Scenario: A link names its organization
- **WHEN** any surface writes a link to an object
- **THEN** the link is at the host's one address with the organization in the
  path (205a)

### Requirement: One GitHub App is the connection, and its key is the operator's to place

One GitHub App SHALL be the connection, and its installation SHALL cover every
repository the manifest lists; adding a repository SHALL extend the installation
or surface as a decision under attention (207). No host and no session SHALL use
a personal token (207). A session in a place SHALL be given a short-lived
installation token scoped to that repository, issued into the place and written
nowhere else (207); with the operator as the session binding and the
line-and-place effects recorded there is no place on disk to issue one into, so
that sentence is proved in the phase that has both (93a, 93b). A self-managed
organization SHALL use its own App, its key placed by the operator and never by
an agent (207a).

#### Scenario: A repository the installation does not cover
- **WHEN** the manifest lists a repository the App's installation does not cover
- **THEN** a decision under attention says which repository, and the machinery
  takes nothing on it until the installation is extended (207)

#### Scenario: The App's key is placed, never written by the machinery
- **WHEN** the App's key is needed
- **THEN** it is read from where the operator placed it, and no configuration or
  code of the machinery holds it (207, 207a)

### Requirement: The new flywheel's objects are its own, in its own state repository

The new flywheel SHALL run beside the existing one against the same
organization, without either interfering with the other, and its scope of
objects SHALL be disjoint and explicit (96). Its objects SHALL live in its own
state repository, and it SHALL touch nothing of the existing flywheel's (96).

#### Scenario: The two do not meet
- **WHEN** both flywheels run against the same organization
- **THEN** the new one reads and writes only its own state repository and the
  prefix it owns, and nothing of the existing one's is read or written (96)

### Requirement: A repository record holds its git details alone

A repository record SHALL hold the repository's git details alone and SHALL
never hold a hand-declared kind, capability or scope (199).

#### Scenario: Nothing is declared by hand
- **WHEN** a repository is registered
- **THEN** its record names its git details, and no kind, capability or scope
  appears on it (199)

### Requirement: An organization is removed by a response, never by deleting files

An organization SHALL be removed by a response and never by deleting files: its
sessions SHALL end, its places SHALL be removed, its state SHALL be archived,
its git repositories SHALL be left on disk, and its numbers SHALL never be
reused (221, 15, 4).

#### Scenario: The operator dictates the removal
- **WHEN** the operator dictates the organization's removal
- **THEN** its sessions end, its places are removed, its state is archived with
  its decision counter, its repositories stay on disk, and no number it issued
  is ever reused (221, 15, 4)

### Requirement: The template set is versioned and its version is stamped

The blueprints template, the built-repository template and the shipped skills
and deliverables SHALL be one versioned set released with the flywheel (208).
Initialization and repository creation SHALL stamp the version they used (208).

#### Scenario: The set version is recorded at initialization
- **WHEN** an organization is initialized or a repository is created
- **THEN** the version of the set used is recorded with it (208)

#### Scenario: A newer set upgrades nothing on its own
- **WHEN** a newer set exists than the one a repository was stamped with
- **THEN** the stamp is unchanged and nothing is upgraded; that the upgrade is a
  chore the operator accepts is 208's rule and belongs to the phase that has
  chores

### Requirement: Three repositories, three owners, and the machinery writes only under its prefix

The state repository SHALL be the machinery's alone; nothing a person or a
session writes SHALL live there (203). In the blueprints repository the
machinery SHALL write only under its own prefix and in the change directory it
creates when an intent opens (203). In a built repository it SHALL write only
the changes for units, the acceptance file, and an untracked per-place directory
(203). The machinery SHALL never write outside its prefix except as the effect
of a response (203).

#### Scenario: A write outside the prefix
- **WHEN** an effect would write a tracked file in the blueprints repository
  outside the machinery's prefix and outside an intent's change directory
- **THEN** it is performed only as the effect of a response, and otherwise
  refused and reported (203)

#### Scenario: Nothing of a person's lives in the state repository
- **WHEN** the state repository is read
- **THEN** it holds only the machinery's own records in the profile's layout
  (203)
