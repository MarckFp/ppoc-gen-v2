# Project Specification: Local-First Scheduling App

---

## **1. Project Overview & Core Principles**

This document outlines the specifications for a **local-first, private, and multi-platform scheduling application**. The primary goal is to provide a tool for creating and managing publisher schedules for a congregation.

The application is built on the following core principles:

* **Offline First** 🌐: The app must be fully functional without an internet connection. All data and logic reside on the user's device.
* **Privacy & Security** 🔒: All user data is stored locally in an encrypted format. No data is sent to external servers, ensuring complete user privacy.
* **Performance** 🚀: The application must be fast and responsive, providing a smooth user experience.
* **Multi-Platform** 💻📱: The app will be built once and deployed across major desktop and mobile platforms (Windows, macOS, Linux, iOS, Android).

---

## **2. Technology Stack**

* **Core Framework**: **Dioxus** (with Rust) to enable fast, reliable, cross-platform development.
* **Database**: **SQLite** for a portable, single-file, relational database solution. The database file should be encrypted (e.g., using `SQLCipher`).
* **UI Components**: A **shadcn-ui**-like component library for a clean, modern, and accessible user interface.
* **Deployment**: Handled via GitHub Actions (out of scope for this document).

---

## **3. Database Schema & Data Models**

The application will use a single SQLite database file. The schema is designed to be relational to ensure data integrity.



### **Table: `Configuration`**
Stores one-time setup and user preferences. Contains a single row.

| Column Name | Data Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER | Primary Key (Always 1) |
| `congregation_name` | TEXT | The name of the congregation. |
| `theme` | TEXT | User's preferred theme ('Light', 'Dark', 'System'). |

### **Table: `Publishers`**
Stores information about each individual.

| Column Name | Data Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER | Primary Key, Auto-increment |
| `first_name` | TEXT | Publisher's first name. |
| `last_name` | TEXT | Publisher's last name. |
| `gender` | TEXT | 'Male' or 'Female'. |
| `is_shift_manager` | BOOLEAN | `true` if they can act as a shift manager. |
| `priority` | INTEGER | Scheduling priority (1 is highest). |

### **Table: `Schedules`**
Defines the recurring templates for shifts.

| Column Name | Data Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER | Primary Key, Auto-increment |
| `location` | TEXT | e.g., "Main Square Cart". |
| `start_hour` | TEXT | Start time in HH:MM format (e.g., "10:00"). |
| `end_hour` | TEXT | End time in HH:MM format (e.g., "12:00"). |
| `weekday` | TEXT | 'Monday', 'Tuesday', etc. |
| `description` | TEXT | Optional details about the schedule. |
| `num_publishers` | INTEGER | Total number of publishers required for this shift. |
| `num_shift_managers`| INTEGER | Required number of shift managers. |
| `num_brothers` | INTEGER | Required number of male publishers (non-managers). |
| `num_sisters` | INTEGER | Required number of female publishers. |
**Constraint**: The UI and backend must enforce `$N_{managers} + N_{brothers} + N_{sisters} \le N_{publishers}$`.

### **Table: `Absences`**
Tracks when publishers are unavailable.

| Column Name | Data Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER | Primary Key, Auto-increment |
| `publisher_id` | INTEGER | Foreign Key to `Publishers.id`. |
| `start_date` | TEXT | Start of absence (ISO 8601 format: YYYY-MM-DD). |
| `end_date` | TEXT | End of absence (ISO 8601 format: YYYY-MM-DD). |
| `description` | TEXT | Optional reason for absence. |
**Behavior**: A background task or startup check should delete records where `end_date` is before the current date.

### **Table: `Shifts`**
Stores the generated, concrete instances of a schedule with assigned publishers.

| Column Name | Data Type | Description |
| :--- | :--- | :--- |
| `id` | INTEGER | Primary Key, Auto-increment |
| `start_datetime` | TEXT | Full start date and time (ISO 8601). |
| `end_datetime` | TEXT | Full end date and time (ISO 8601). |
| `location` | TEXT | The location of the shift. |
| `publishers` | TEXT | A JSON array of `publisher_id`s assigned to this shift. `[]` if unassigned. |
| `warning` | TEXT | A warning message if the shift could not be filled correctly. |

### **Join Tables (Many-to-Many Relationships)**

* **`Availability` (`Publisher_Schedules`)**: Links publishers to the schedules they are available for.
    * `publisher_id`: Foreign Key to `Publishers.id`.
    * `schedule_id`: Foreign Key to `Schedules.id`.
* **`Relationships` (`Publisher_Relationships`)**: Links publishers who prefer to work together.
    * `publisher_a_id`: Foreign Key to `Publishers.id`.
    * `publisher_b_id`: Foreign Key to `Publishers.id`.

---

## **4. Application Screens & Features**

### **Initial Setup**
* On first launch, the app presents a modal or dedicated page.
* **Inputs**: "Congregation Name" and "Theme" (Radio buttons: Light, Dark, System).
* This data is saved to the `Configuration` table. These settings can be changed later in a "Configuration" section of the app.

### **Main Sections (CRUD Interfaces)**
Each of the following sections will have a dedicated view with a table/list display and functionality for **C**reate, **R**ead, **E**dit, and **D**elete.

1.  **Publishers**:
    * Display a searchable, sortable list of all publishers.
    * **CRUD**: Forms for creating/editing single publishers.
    * **Bulk Edit**: Allow selecting multiple publishers to change `priority` or `is_shift_manager` status simultaneously.
    * **Delete**: Deleting a publisher requires a confirmation modal and will cascade to remove their related `Availability`, `Relationships`, and `Absences`.
2.  **Schedules**:
    * Display a list of all schedule templates.
    * **CRUD**: Forms for creating/editing schedules. The form must include validation to ensure the sum of specific publisher types does not exceed the total `num_publishers`.
3.  **Absences**:
    * Display a list of current and future absences.
    * **CRUD**: Forms for creating/editing absences, including a publisher selector and date pickers.

### **General UI/UX**
* **Confirmation Modals**: All `DELETE` operations must trigger a modal asking "Are you sure?" to prevent accidental data loss.
* **Navigation**: A simple sidebar or tab bar to navigate between Publishers, Schedules, Absences, and Shifts sections.

---

## **5. Core Logic: Shift Generation & Export**

This is the most complex feature, accessed via a "Generate Shifts" button in the "Shifts" section.

### **User Interaction**
1.  User clicks "Generate Shifts".
2.  A modal appears asking for a **date range**.
    * **Options**: Custom `start_date` and `end_date` pickers.
    * **Presets**: Buttons for "This Week", "Next Week", "This Month", "Next Month".
3.  Upon confirmation, a loading indicator is shown while the algorithm runs.
4.  Generated shifts are added to the `Shifts` table and displayed in the UI.

### **Algorithm Options**
The system can provide two generation methods. The choice can be a user setting or two separate buttons.

#### **Method A: Local AI Model (Advanced)** 🤖
* **Concept**: Use a small, efficient AI model (like a quantized version of a smaller LLM) running locally on the device.
* **Process**:
    1.  **Context Assembly**: Collect all relevant data (Publishers, Schedules, Absences, Availability, Relationships, and existing Shifts in the period) and format it into a structured text or JSON prompt.
    2.  **Prompting**: Feed the context to the local AI model with a clear instruction, e.g., *"Generate a fair schedule for the given date range. Fulfill all schedule requirements. Prioritize publishers with a higher priority number. Respect publisher absences and availability. Try to pair publishers who have a relationship. Output the result as a JSON array of shift objects."*
    3.  **Parsing**: Parse the model's JSON output.
    4.  **Validation & Insertion**: Validate the returned shifts (e.g., check for correct publisher IDs) and insert them into the `Shifts` database table. Handle potential parsing errors or invalid outputs gracefully.

#### **Method B: Algorithmic (Deterministic)** ⚙️
* **Concept**: A rule-based algorithm that iterates through days and schedules to assign publishers based on a scoring system.
* **Step-by-Step Process**:
    1.  **Initialization**: For the given date range, create empty shift slots for every applicable `Schedule` on its corresponding `weekday`.
    2.  **Publisher Pool**: Fetch all `Publishers`. For each publisher, calculate a "fairness score" based on the number of shifts they've been assigned recently (e.g., in the last 30 days). A lower score is better.
    3.  **Iteration**: Loop through each empty shift slot chronologically.
    4.  **Candidate Filtering**: For the current empty shift, find all potential `Publisher` candidates who:
        * Are available for that `Schedule` (via `Availability` table).
        * Do not have an `Absence` on that day.
        * Are not already assigned to another shift on the same day.
    5.  **Candidate Scoring**: Score each candidate based on a weighted formula:
        * `Score = (Priority * w1) + (FairnessScore * w2) + (RelationshipBonus * w3)`
        * **Priority**: Higher priority (`1`) gives a better score.
        * **FairnessScore**: A lower number of recent shifts gives a better score.
        * **RelationshipBonus**: A bonus is awarded if another publisher from their `Relationships` list is already selected for this shift.
    6.  **Assignment**:
        * From the scored candidates, fill the required slots (`num_shift_managers`, `num_brothers`, `num_sisters`) by picking the highest-scoring candidates that match the gender/role criteria.
        * Fill the remaining general `num_publishers` slots from the rest of the top-scoring candidates.
    7.  **Finalization**:
        * If a shift is filled successfully, save the list of `publisher_id`s to the `Shifts` record.
        * If any slot cannot be filled, save the partially filled shift and add a `warning` message (e.g., "Could not find an available Shift Manager.").
        * Update the "fairness score" for all assigned publishers.
    8.  **Repeat** until all shifts in the range are processed.

### **Export Functionality**
* The "Shifts" view will have an "Export" button.
* The user can select a date range (similar to generation).
* **Output Formats**: PDF and Excel (`.xlsx`).
* **Content**: The export should be a well-formatted, human-readable schedule, grouped by day. Each entry should list the `start_datetime`, `end_datetime`, `location`, and the full names of the assigned `publishers`.

---

## **6. Data Management: Import & Export**

* **Export**:
    * An "Export Data" button in the "Configuration" section.
    * Exports all tables (`Publishers`, `Schedules`, `Absences`, `Availability`, `Relationships`, `Shifts`) into a single, encrypted file (e.g., a `.json` or `.db` file).
    * The `Configuration` table is **explicitly excluded**.
* **Import**:
    * An "Import Data" button.
    * Prompts the user to select the exported file.
    * Displays a prominent **confirmation modal**: "This will overwrite all existing publishers, schedules, and shifts. This action cannot be undone. Are you sure you want to proceed?".
    * On confirmation, it drops all existing data (except from `Configuration`) and imports the data from the file.