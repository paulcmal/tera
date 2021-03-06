use errors::Result;
use parser::ast::MacroDefinition;
use std::collections::HashMap;
use template::Template;
use tera::Tera;

// Types around Macros get complicated, simplify it a bit by using aliases

/// Maps { macro => macro_definition }
pub type MacroDefinitionMap = HashMap<String, MacroDefinition>;
/// Maps { namespace => ( macro_template, { macro => macro_definition }) }
pub type MacroNamespaceMap<'a> = HashMap<&'a str, (&'a str, &'a MacroDefinitionMap)>;
/// Maps { template => { namespace => ( macro_template, { macro => macro_definition }) }
pub type MacroTemplateMap<'a> = HashMap<&'a str, MacroNamespaceMap<'a>>;

/// Collection of all macro templates by file
#[derive(Clone, Debug, Default)]
pub struct MacroCollection<'a> {
    macros: MacroTemplateMap<'a>,
}

impl<'a> MacroCollection<'a> {
    pub fn from_original_template(tpl: &'a Template, tera: &'a Tera) -> MacroCollection<'a> {
        let mut macro_collection = MacroCollection { macros: MacroTemplateMap::new() };

        macro_collection
            .add_macros_from_template(tera, tpl)
            .expect("Couldn't load macros from base template");

        macro_collection
    }

    /// Add macros from parsed template to `MacroCollection`
    ///
    /// Macro templates can import other macro templates so the macro loading needs to
    /// happen recursively. We need all of the macros loaded in one go to be in the same
    /// HashMap for easy popping as well, otherwise there could be stray macro
    /// definitions remaining
    /// TODO: add checks while building Tera that all the template files with macros are loaded
    /// so we can get rid of Result here
    pub fn add_macros_from_template(
        self: &mut Self,
        tera: &'a Tera,
        template: &'a Template,
    ) -> Result<()> {
        let template_name = &template.name[..];
        if self.macros.contains_key(template_name) {
            return Ok(());
        }

        let mut macro_namespace_map = MacroNamespaceMap::new();

        if !template.macros.is_empty() {
            macro_namespace_map.insert("self", (template_name, &template.macros));
        }

        for &(ref filename, ref namespace) in &template.imported_macro_files {
            let macro_tpl = tera.get_template(filename)?;
            macro_namespace_map.insert(namespace, (filename, &macro_tpl.macros));
            self.add_macros_from_template(tera, macro_tpl)?;
        }

        self.macros.insert(template_name, macro_namespace_map);

        for parent in &template.parents {
            let parent = &parent[..];
            let parent_template = tera.get_template(parent)?;
            self.add_macros_from_template(tera, parent_template)?;
        }

        Ok(())
    }

    pub fn lookup_macro(
        &self,
        template_name: &'a str,
        macro_namespace: &'a str,
        macro_name: &'a str,
    ) -> Result<(&'a str, &'a MacroDefinition)> {
        let namespace = self
            .macros
            .get(template_name)
            .and_then(|namespace_map| namespace_map.get(macro_namespace));

        if let Some(n) = namespace {
            let &(macro_template, macro_definition_map) = n;

            if let Some(m) = macro_definition_map.get(macro_name).map(|md| (macro_template, md)) {
                Ok(m)
            } else {
                bail!(
                    "Macro `{}::{}` not found in template `{}`",
                    macro_namespace,
                    macro_name,
                    template_name
                )
            }
        } else {
            bail!(
            "Macro namespace `{}` was not found in template `{}`. Have you maybe forgotten to import it, or misspelled it?",
            macro_namespace, template_name
            )
        }
    }
}
