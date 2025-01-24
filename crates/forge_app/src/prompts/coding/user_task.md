<task>{{task}}</task>

{{#each files}}
<file_content path="{{this.path}}">
{{this.content}}
</file_content>
{{/each}}

<workspace id="{{workspace.workspace_id}}">
<focused_file>{{workspace.focused_file}}</focused_file>
{{#each workspace.opened_files}}
<opened_file>{{this}}</opened_file>
{{/each}}
</workspace>
